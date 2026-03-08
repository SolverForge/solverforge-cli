# Operating Room Scheduler - 5 Minute Demo Script

## Pre-Recording Setup
```bash
cd /srv/lab/dev/solverforge/solverforge-cli
./demo-files/video-setup.sh
```

## Recording Script

### Opening (0:00-0:30)
```bash
cd ~/or-scheduling-demo
ls -la
```
"I'm building a hospital OR scheduler from scratch using only CLI commands. No typing code. Watch this."

### Part 1: Domain Modeling (0:30-2:30)

```bash
# Create solution (what we're solving for)
solverforge generate solution or_schedule
```
*While running:* "The solution holds our answer - all surgeries assigned to rooms and times."

```bash
# Create surgery entity with first planning variable
solverforge generate entity surgery --planning-variable room_idx
```
*While running:* "Entities need assignment. Planning variables are what the solver figures out - which room."

```bash
# Add second planning variable
solverforge generate variable slot_idx --entity Surgery
```
*While running:* "Adding time slot assignment. The solver will assign both room AND time."

```bash
# Quick peek at auto-generated code
cat src/domain/surgery.rs | head -14
```
"Both variables wired up, ready for the solver."

```bash
# Create facts (fixed resources)
solverforge generate fact operating_room
solverforge generate fact time_slot
```
*While running:* "Facts are our resources - 6 operating rooms, 8 time slots. They don't change."

```bash
# Show auto-wiring
grep -n "collection" src/domain/or_schedule.rs
```
"Everything auto-wired into the solution. Zero manual connections."

### Part 2: Constraints (2:30-4:00)

```bash
# No overlapping surgeries (use wizard)
solverforge generate constraint no_overlap
```
*Navigate wizard:*
- Select: "Penalize conflicting pairs"
- Select: "Hard"

*While navigating:* "Hard constraint - no double-booking rooms."

```bash
# Fast-generate remaining constraints
solverforge generate constraint equipment_match --join --hard
```
"Room must have required equipment."

```bash
solverforge generate constraint surgeon_available --join --hard
```
"Surgeon must be available."

```bash
solverforge generate constraint priority_first --reward
```
"Soft constraint - prefer scheduling emergencies first."

```bash
solverforge generate constraint maximize_utilization --balance
```
"Minimize gaps between surgeries."

```bash
# Show all constraints registered
grep "pub fn" src/constraints/mod.rs
```
"Five constraints, all auto-registered."

### Part 3: Add Implementation & Run (4:00-5:00)

```bash
# Copy pre-prepared implementations
cp ../demo-files/surgery-complete.rs src/domain/surgery.rs
cp ../demo-files/operating-room-complete.rs src/domain/operating_room.rs  
cp ../demo-files/time-slot-complete.rs src/domain/time_slot.rs
cp ../demo-files/constraints-complete/*.rs src/constraints/
cp ../demo-files/demo-data.rs src/data/mod.rs
```
"Adding domain fields and constraint logic. The scaffolding handles all wiring."

```bash
# Build and run
cargo build --release
solverforge server &
sleep 2
```

```bash
# Solve via API
curl -X POST localhost:7860/solve | jq '.score'
```
"Zero hard violations! 30 surgeries scheduled."

```bash
# Show assignments
curl localhost:7860/solve | jq '.solution.surgeries[0:3] | .[] | {id, room_idx, slot_idx, priority}'
```
"Each surgery assigned to room and time slot. Emergencies scheduled first."

### Closing (4:45-5:00)
"Five minutes. Ten commands. Zero manual wiring. This compiles to a single binary - no JVM, no cloud dependency. Hospitals pay millions for this."

## Key Points to Emphasize
- **10 CLI commands** generate entire domain model
- **Auto-wiring** - no manual connections needed
- **Pure Rust** - compiles to single binary
- **Production-ready** - handles real hospital constraints
- **No code typing** - just domain knowledge needed

## Backup Commands (if time permits)
```bash
# Show a specific surgery assignment
curl localhost:7860/solve | jq '.solution.surgeries[] | select(.priority == 1)'

# Show solver statistics
curl localhost:7860/solve | jq '.stats'

# Count total generated lines
find src -name "*.rs" | xargs wc -l | tail -1
```