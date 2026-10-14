#!/usr/bin/env bash
#
# Run the LB+UAF harness (src/bin/harness.rs) on an AWS Graviton (AArch64)
# instance and pull the results back.
#
# Two questions: does the program as written ever show (1,1) on Arm, where the
# ISA allows load buffering; and does the reordered program show the
# use-after-free there, natively and under AddressSanitizer. It also runs the
# exhibit 2 harness (src/bin/mp.rs): relaxed message passing, which Arm
# produces and x86 cannot.
#
# SELF-CONTAINED and EPHEMERAL: a throwaway key pair, a security group locked
# to your public IP, one instance with a 60-minute self-terminate, and an EXIT
# trap that deletes all of it. Results land in aarch64/results/.
#
# Prerequisites: aws CLI (configured), ssh, scp, curl, jq. A few cents of EC2.
#
# Usage:
#   aarch64/run-graviton.sh                            # c7g.2xlarge (Graviton3)
#   INSTANCE_TYPE=c8g.2xlarge aarch64/run-graviton.sh  # Graviton4
#   WRITTEN_RUNS=1000000000 aarch64/run-graviton.sh    # longer LB search
#
set -euo pipefail

# ----------------------------------------------------------------------------
# Config (override via environment)
# ----------------------------------------------------------------------------
REGION="${REGION:-$(aws configure get region 2>/dev/null || echo us-east-1)}"
INSTANCE_TYPE="${INSTANCE_TYPE:-c7g.2xlarge}"   # Graviton3, 8 vCPU. c8g=Graviton4.
WRITTEN_RUNS="${WRITTEN_RUNS:-200000000}"      # LB search on the program as written
REORDERED_RUNS="${REORDERED_RUNS:-10000000}"   # per batch, three batches
MP_RUNS="${MP_RUNS:-100000000}"                # exhibit 2, each ordering
SSH_USER="ec2-user"                              # Amazon Linux 2023
TAG="lb-uaf-graviton"

# A short run id. We avoid relying on a precise clock; seconds-since-epoch is fine.
RUN_ID="$(date +%Y%m%d-%H%M%S)"
HERE="$(cd "$(dirname "$0")/.." && pwd)"         # repo root
RESULTS_DIR="$HERE/aarch64/results"
RESULTS="$RESULTS_DIR/graviton-$RUN_ID.txt"
WORK="$(mktemp -d)"
KEY_NAME="$TAG-$RUN_ID"
SG_NAME="$TAG-$RUN_ID"
PEM="$WORK/$KEY_NAME.pem"

mkdir -p "$RESULTS_DIR"

say() { printf '\033[1;36m==>\033[0m %s\n' "$*" >&2; }
warn() { printf '\033[1;33m!! \033[0m %s\n' "$*" >&2; }

# ----------------------------------------------------------------------------
# Cleanup trap — runs no matter how we exit.
# ----------------------------------------------------------------------------
INSTANCE_ID=""
SG_ID=""
KEY_CREATED=""
cleanup() {
  local code=$?
  say "Cleaning up AWS resources…"
  if [[ -n "$INSTANCE_ID" ]]; then
    aws ec2 terminate-instances --region "$REGION" --instance-ids "$INSTANCE_ID" >/dev/null 2>&1 || true
    aws ec2 wait instance-terminated --region "$REGION" --instance-ids "$INSTANCE_ID" 2>/dev/null || true
    say "  terminated $INSTANCE_ID"
  fi
  if [[ -n "$SG_ID" ]]; then
    # The ENI can linger a few seconds after termination; retry the SG delete.
    for _ in $(seq 1 20); do
      if aws ec2 delete-security-group --region "$REGION" --group-id "$SG_ID" >/dev/null 2>&1; then
        say "  deleted security group $SG_ID"; break
      fi
      sleep 3
    done
  fi
  if [[ -n "$KEY_CREATED" ]]; then
    aws ec2 delete-key-pair --region "$REGION" --key-name "$KEY_NAME" >/dev/null 2>&1 || true
    say "  deleted key pair $KEY_NAME"
  fi
  rm -rf "$WORK"
  exit $code
}
trap cleanup EXIT INT TERM

# ----------------------------------------------------------------------------
# Preflight
# ----------------------------------------------------------------------------
for bin in aws ssh scp curl jq; do
  command -v "$bin" >/dev/null || { warn "missing required tool: $bin"; exit 1; }
done
aws sts get-caller-identity >/dev/null 2>&1 || { warn "AWS credentials not configured"; exit 1; }
say "Region $REGION | instance $INSTANCE_TYPE | written $WRITTEN_RUNS, reordered 3 x $REORDERED_RUNS"

# Latest Amazon Linux 2023 arm64 AMI (resolved from the public SSM parameter).
AMI_ID="$(aws ssm get-parameters --region "$REGION" \
  --names /aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-arm64 \
  --query 'Parameters[0].Value' --output text)"
[[ "$AMI_ID" == ami-* ]] || { warn "could not resolve AL2023 arm64 AMI"; exit 1; }
say "AMI $AMI_ID (Amazon Linux 2023, arm64)"

# Default VPC (needed to host the throwaway security group).
VPC_ID="$(aws ec2 describe-vpcs --region "$REGION" \
  --filters Name=isDefault,Values=true --query 'Vpcs[0].VpcId' --output text)"
[[ "$VPC_ID" == vpc-* ]] || { warn "no default VPC in $REGION — set one up or pick another region"; exit 1; }

MY_IP="$(curl -fsS https://checkip.amazonaws.com)"
say "Locking SSH ingress to $MY_IP/32"

# ----------------------------------------------------------------------------
# Provision
# ----------------------------------------------------------------------------
say "Creating key pair…"
aws ec2 create-key-pair --region "$REGION" --key-name "$KEY_NAME" \
  --query 'KeyMaterial' --output text > "$PEM"
chmod 600 "$PEM"
KEY_CREATED=1

say "Creating security group…"
SG_ID="$(aws ec2 create-security-group --region "$REGION" \
  --group-name "$SG_NAME" --description "ephemeral SSH for $TAG" \
  --vpc-id "$VPC_ID" --query 'GroupId' --output text)"
aws ec2 authorize-security-group-ingress --region "$REGION" \
  --group-id "$SG_ID" --protocol tcp --port 22 --cidr "$MY_IP/32" >/dev/null

say "Launching instance…"
# Safety net: if this local script is SIGKILLed and the EXIT trap never runs,
# the instance self-terminates in 60 min (shutdown + shutdown-behavior=terminate).
SELF_DESTRUCT="$(printf '#!/bin/bash\nshutdown -h +60\n')"
INSTANCE_ID="$(aws ec2 run-instances --region "$REGION" \
  --image-id "$AMI_ID" --instance-type "$INSTANCE_TYPE" \
  --key-name "$KEY_NAME" --security-group-ids "$SG_ID" \
  --instance-initiated-shutdown-behavior terminate \
  --user-data "$SELF_DESTRUCT" \
  --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=$TAG}]" \
  --query 'Instances[0].InstanceId' --output text)"
say "  $INSTANCE_ID — waiting for it to come up…"
aws ec2 wait instance-running --region "$REGION" --instance-ids "$INSTANCE_ID"

PUBLIC_IP="$(aws ec2 describe-instances --region "$REGION" \
  --instance-ids "$INSTANCE_ID" \
  --query 'Reservations[0].Instances[0].PublicIpAddress' --output text)"
say "  public IP $PUBLIC_IP — waiting for SSH…"

SSH_OPTS=(-i "$PEM" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null
          -o ConnectTimeout=10 -o LogLevel=ERROR)
for _ in $(seq 1 40); do
  if ssh "${SSH_OPTS[@]}" "$SSH_USER@$PUBLIC_IP" true 2>/dev/null; then break; fi
  sleep 5
done
ssh "${SSH_OPTS[@]}" "$SSH_USER@$PUBLIC_IP" true 2>/dev/null \
  || { warn "SSH never became reachable"; exit 1; }
say "SSH is up."

# ----------------------------------------------------------------------------
# Ship the crate, then build and run remotely
# ----------------------------------------------------------------------------
say "Uploading source…"
TARBALL="$WORK/src.tgz"
tar -C "$HERE" -czf "$TARBALL" Cargo.toml Cargo.lock src
scp "${SSH_OPTS[@]}" "$TARBALL" "$SSH_USER@$PUBLIC_IP:/tmp/src.tgz" >/dev/null

say "Installing Rust, building, and running on Graviton…"
ssh "${SSH_OPTS[@]}" "$SSH_USER@$PUBLIC_IP" \
  "WRITTEN_RUNS='$WRITTEN_RUNS' REORDERED_RUNS='$REORDERED_RUNS' MP_RUNS='$MP_RUNS' INSTANCE_TYPE='$INSTANCE_TYPE' bash -s" \
  <<'REMOTE' 2>&1 | tee "$RESULTS"
set -euo pipefail
echo "================ remote host / hardware ================"
echo "ec2 instance type : ${INSTANCE_TYPE:-?}"
uname -a
part=$(awk -F': ' '/CPU part/{print $2; exit}' /proc/cpuinfo)
case "$part" in
  0xd0c) uarch="Neoverse-N1 (Graviton2)";;
  0xd40) uarch="Neoverse-V1 (Graviton3)";;
  0xd4f) uarch="Neoverse-V2 (Graviton4)";;
  *)     uarch="unknown ARM part $part";;
esac
echo "cpu microarch     : $uarch (part=$part)"
nproc
echo "======================================================="

command -v cc >/dev/null 2>&1 || sudo dnf -y install gcc binutils >/dev/null 2>&1
if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal >/dev/null 2>&1
fi
source "$HOME/.cargo/env"
rustup toolchain install nightly --profile minimal --component rust-src >/dev/null 2>&1
echo "--- toolchains ---"; rustc -V; rustc +nightly -V; echo "------------------"

mkdir -p ~/lbuaf && tar -C ~/lbuaf -xzf /tmp/src.tgz && cd ~/lbuaf
cargo build --release --bin harness --bin mp 2>&1 | tail -1
RUSTFLAGS="-Zsanitizer=address" cargo +nightly build --release --bin harness \
  --target aarch64-unknown-linux-gnu 2>&1 | tail -1

echo
echo "################ native harness ################"
./target/release/harness written "$WRITTEN_RUNS"
for i in 1 2 3; do ./target/release/harness reordered "$REORDERED_RUNS"; done

echo
echo "################ exhibit 2: message passing ################"
./target/release/mp relaxed "$MP_RUNS"
./target/release/mp release-acquire "$MP_RUNS"

echo
echo "################ AddressSanitizer ################"
A=target/aarch64-unknown-linux-gnu/release/harness
"$A" written "$REORDERED_RUNS" 2>&1 | tail -3 || true
echo "--- reordered ---"
( time "$A" reordered "$REORDERED_RUNS" ) 2>&1 | head -12 || true
REMOTE

say "Results saved to: $RESULTS"
