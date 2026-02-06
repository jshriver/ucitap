<div align="center">
# ucitap
</div>

Chess UCI protocol GUI to Engine Logger written in Rust.  You can install Rust with the instructions [here](https://rust-lang.org/tools/install/).
It also include ucitap2json which takes the recorded raw log and exports a json of the position and associated info line variables.

# Build instructions:
```
git clone https://github.com/jshriver/ucitap.git
cd ucitap
cargo build --release
```

# Usage:

Included is a sample config.json which is the default file used to spawn the target chess engine. You can also specify using the --config command line argument.

Sample config:
```
{
  "engine": "./stockfish",
  "logfile": "stockfish.log"
}
````

Wherever you wish to use the engine, simple run ucitap in it's place. All i/o between the GUI and the engine will be recorded in the logfile specified in the config.

# ucitap2json
Sample json export from the ucitap logs. Moves are converted from uci to san format.
```
[
  {
    "engine": "Stockfish 18",
    "fen": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -",
    "ply": 30,
    "score": 34,
    "mate": null,
    "nodes": 24927638,
    "nps": 1474223,
    "time": 16909,
    "pv": "d4 Nf6 c4 e6 g3 d5 Nf3 Nc6 Bg2 dxc4 Qa4 Bb4 Bd2 Nd5 Bxb4 Nxb4"
  },
  {
    "engine": "Stockfish 18",
    "fen": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -",
    "ply": 31,
    "score": 35,
    "mate": null,
    "nodes": 13622132,
    "nps": 1719097,
    "time": 7924,
    "pv": "d4 d5 c4 e6 Nf3 dxc4 e3 c5 Bxc4 a6"
  }
]
```

These were created primarily for my OpenchessDB project.  This provides an easy way to record data whenever manually analysing positions, playing games against an engine or doing engine matches for ratings lists. 