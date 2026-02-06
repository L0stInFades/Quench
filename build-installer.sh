#!/bin/bash

# ZipX Windows Installer Build Script

echo "🚀 Building ZipX Windows Installer..."

# Navigate to UI directory
cd ui || exit 1

# Check if npm is installed
if ! command -v npm &> /dev/null; then
    echo "❌ Error: npm not found. Please install Node.js first."
    echo "   Download from: https://nodejs.org/"
    exit 1
fi

echo "✅ npm found: $(npm --version)"

# Install dependencies
echo "📦 Installing npm dependencies..."
if [ ! -d "node_modules" ]; then
    npm install || {
        echo "❌ Failed to install dependencies"
        exit 1
    }
fi

# Build Tauri application
echo "🔨 Building Tauri application (this may take a few minutes)..."
npm run tauri build || {
    echo "❌ Build failed"
    exit 1
}

# Check if build succeeded
if [ -f "src-tauri/target/release/zipx-ui.exe" ]; then
    echo "✅ Build successful!"
    echo "📍 Location: ui/src-tauri/target/release/zipx-ui.exe"
    echo ""
    echo "📦 Installer bundles:"
    find src-tauri/target/release/bundle -type f -name "*.exe" -o -name "*.msi" 2>/dev/null || echo "No installer bundles found"
else
    echo "❌ Build output not found"
    exit 1
fi

echo ""
echo "🎉 ZipX Windows installer build complete!"
