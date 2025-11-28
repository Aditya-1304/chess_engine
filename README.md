# AdiChess Engine

AdiChess is a strong, UCI-compatible chess engine written in Rust. This is the **first stable release v0.1** of the engine, featuring advanced search techniques and efficient evaluation.

## Features

*   **UCI Compatible**: Works with any standard chess GUI (Arena, CuteChess, Lichess-bot, etc.).
*   **Advanced Search**: Alpha-Beta pruning, Principal Variation Search (PVS), Null Move Pruning, Late Move Reductions (LMR).
*   **Efficient Evaluation**: evaluation with Halfkp NNUE (Stockfish 13 NNUE architecture).
*   **Transposition Table**: High-performance, lock-less transposition table.
*   **Parallel Search**: Supports multi-threaded search (Lazy SMP).

## Installation

### Option 1: Download Binary (Recommended)

You can download the pre-compiled binary for your operating system from the **[Releases](../../releases)** section of this repository.

1.  Go to the [Releases](../../releases) page.
2.  Download the latest version for your OS (Linux or Windows).
3.  Extract the file and point your Chess GUI to the executable.

### Option 2: Build from Source

If you prefer to build the engine yourself, you will need [Rust](https://www.rust-lang.org/tools/install) installed.

1.  Clone the repository:
    ```bash
    git clone https://github.com/YOUR_USERNAME/chess_engine.git
    cd chess_engine
    ```

2.  Build the engine in release mode:
    ```bash
    cargo build --release
    ```

3.  The executable will be located at release section of repository:
    *   **Linux/Mac**: `target/release/chess_engine_linux`
    *   **Windows**: `target/release/chess_engine.exe`

## Usage

AdityaChess is a command-line engine and does not have its own graphical user interface (GUI). You should use it with a chess GUI like Arena, BanksiaGUI, or CuteChess, or connect it to Lichess using `lichess-bot`.

### Using with Lichess-Bot

To connect the engine to Lichess, you can use `lichess-bot`.
Follow the official instructions here: [lichess-bot-devs/lichess-bot](https://github.com/lichess-bot-devs/lichess-bot)

### Required Files for Full Potential

To ensure the engine plays at its maximum strength, you need to place the following files in the same directory as the engine executable (e.g., inside `target/release/`):

1.  **NNUE Network File** (Required)
    *   **File Name**: `nn-62ef826d1a6d.nnue`
    *   **Download**: [Link to Network](https://tests.stockfishchess.org/nns?network_name=nn-62ef826d1a6d&user=)
    *   *Note*: This file is also present in the repository. You must use this exact network file.

2.  **Opening Book** (Recommended)
    *   **File Name**: `Perfect2023.bin`
    *   *Note*: This file is included in the repository.

3.  **Syzygy Endgame Tablebases** (Optional)
    *   **Download**: [Lichess Tablebases](https://tablebase.lichess.ovh/tables/standard/)
    *   **Recommendation**: Download 3, 4, 5 and 6-piece (only if you want super good endgames as they consume a lot of space and memory, 2-3-5 is fine) tables.
    *   *Warning*: 7-piece tables require terabytes of RAM. Stick to 5-piece tables for standard use.

## License

[MIT License](LICENSE)
