#!/bin/bash

echo "=== VLC Controller Logging Demo ==="
echo ""

echo "1. Testing with INFO level logging (default):"
echo "   RUST_LOG=info ./target/debug/rust-vlc"
echo ""

echo "2. Testing with DEBUG level logging (shows all messages):"
echo "   RUST_LOG=debug ./target/debug/rust-vlc"
echo ""

echo "3. Testing with WARN level logging (only warnings and errors):"
echo "   RUST_LOG=warn ./target/debug/rust-vlc"
echo ""

echo "4. Testing commands (in another terminal while server is running):"
echo "   # TCP interface:"
echo "   echo 'pause' | nc localhost 55550"
echo "   echo 'play' | nc localhost 55550"
echo ""
echo "   # UDP interface:"
echo "   echo 'pause' | nc -u localhost 55551"
echo "   echo 'play' | nc -u localhost 55551"
echo ""

echo "Note: VLC must be running at least with RC interface for forwarding to work:"
echo "      vlc --intf rc --rc-host 127.0.0.1:54322"
