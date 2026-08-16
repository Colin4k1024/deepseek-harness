#!/bin/bash
# Production deployment script for dsh-rust.
#
# Usage:
#   ./deploy/production-deploy.sh [--method docker|kubernetes|systemd] [--version v1.0.0]

set -euo pipefail
cd "$(dirname "$0")/.."

METHOD="${1:-docker}"
VERSION="${2:-latest}"

echo "=== dsh-rust Production Deployment ==="
echo "Method: $METHOD"
echo "Version: $VERSION"
echo ""

# Pre-flight checks
echo "=== Pre-flight Checks ==="

# Check Rust toolchain
if ! command -v cargo &> /dev/null; then
    echo "Error: Rust toolchain not found"
    exit 1
fi
echo "✓ Rust toolchain: $(rustc --version)"

# Check API key
if [ -z "${DEEPSEEK_API_KEY:-}" ]; then
    echo "Error: DEEPSEEK_API_KEY not set"
    exit 1
fi
echo "✓ API key configured"

# Run tests
echo ""
echo "=== Running Tests ==="
cargo test --workspace
echo "✓ All tests passed"

# Run security audit
echo ""
echo "=== Security Audit ==="
if [ -f scripts/security-audit.sh ]; then
    ./scripts/security-audit.sh || true
fi

# Build release
echo ""
echo "=== Building Release ==="
cargo build --release -p dsh-web -p dsh-headless
echo "✓ Release binaries built"

# Deploy
echo ""
echo "=== Deploying ($METHOD) ==="

case "$METHOD" in
    docker)
        echo "Building Docker image..."
        docker build -t "dsh-rust:$VERSION" -f deploy/docker/Dockerfile .
        echo "✓ Docker image built: dsh-rust:$VERSION"

        echo "Starting services..."
        cd deploy/docker
        docker compose up -d
        echo "✓ Services started"

        echo ""
        echo "=== Deployment Complete ==="
        echo "Web API: http://localhost:3000"
        echo "Health:  http://localhost:3000/health"
        echo "Logs:    docker compose logs -f dsh-web"
        ;;

    kubernetes)
        echo "Deploying to Kubernetes..."
        kubectl apply -f deploy/kubernetes/
        echo "✓ Kubernetes resources applied"

        echo "Waiting for rollout..."
        kubectl rollout status deployment/dsh-web -n dsh --timeout=300s
        echo "✓ Rollout complete"

        echo ""
        echo "=== Deployment Complete ==="
        echo "Service: kubectl get svc dsh-web -n dsh"
        echo "Pods:    kubectl get pods -n dsh"
        ;;

    systemd)
        echo "Installing systemd service..."
        sudo cp deploy/systemd/dsh-web.service /etc/systemd/system/
        sudo systemctl daemon-reload
        sudo systemctl enable dsh-web
        sudo systemctl start dsh-web
        echo "✓ systemd service started"

        echo ""
        echo "=== Deployment Complete ==="
        echo "Status: sudo systemctl status dsh-web"
        echo "Logs:   sudo journalctl -u dsh-web -f"
        ;;

    *)
        echo "Error: Unknown deployment method: $METHOD"
        echo "Usage: $0 [--method docker|kubernetes|systemd] [--version v1.0.0]"
        exit 1
        ;;
esac

echo ""
echo "=== Post-Deployment Verification ==="
sleep 5

if [ "$METHOD" = "docker" ]; then
    if curl -sf http://localhost:3000/health > /dev/null; then
        echo "✓ Health check passed"
    else
        echo "✗ Health check failed"
        exit 1
    fi
fi

echo ""
echo "Deployment complete!"
