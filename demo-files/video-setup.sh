#!/bin/bash
# Video Demo Setup Script for Operating Room Scheduler
# Run this before recording to prepare everything

set -e

echo "=== SolverForge OR Scheduler - Video Demo Setup ==="
echo ""

# Clean up any previous attempt
echo "1. Cleaning up previous demo project..."
rm -rf ~/or-scheduling-demo
cd ~

# Create fresh project
echo "2. Creating fresh OR scheduling project..."
solverforge new or-scheduling-demo --basic
cd or-scheduling-demo

# Pre-compile dependencies for fast builds during demo
echo "3. Pre-compiling dependencies (this will make demo builds instant)..."
cargo build --release > /dev/null 2>&1

echo "4. Demo files are ready in: $(pwd)/../demo-files/"
echo ""
echo "=== READY FOR RECORDING ==="
echo ""
echo "Quick reference for the video:"
echo "--------------------------------"
echo "# Part 1: Domain Modeling"
echo "solverforge generate solution or_schedule"
echo "solverforge generate entity surgery --planning-variable room_idx"  
echo "solverforge generate variable slot_idx --entity Surgery"
echo "solverforge generate fact operating_room"
echo "solverforge generate fact time_slot"
echo ""
echo "# Part 2: Constraints"
echo "solverforge generate constraint no_overlap              # Use wizard"
echo "solverforge generate constraint equipment_match --join --hard"
echo "solverforge generate constraint surgeon_available --join --hard"
echo "solverforge generate constraint priority_first --reward"
echo "solverforge generate constraint maximize_utilization --balance"
echo ""
echo "# Part 3: Copy prepared files"
echo "cp ../demo-files/surgery-complete.rs src/domain/surgery.rs"
echo "cp ../demo-files/operating-room-complete.rs src/domain/operating_room.rs"
echo "cp ../demo-files/time-slot-complete.rs src/domain/time_slot.rs"
echo "cp ../demo-files/constraints-complete/*.rs src/constraints/"
echo "cp ../demo-files/demo-data.rs src/data/mod.rs"
echo ""
echo "# Part 4: Run"
echo "cargo build --release"
echo "solverforge server &"
echo "sleep 2"
echo "curl -X POST localhost:7860/solve | jq '.score, .stats'"
echo ""
echo "Directory is: ~/or-scheduling-demo"
echo "Demo files are in: ~/demo-files/"