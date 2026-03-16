---
title: Performance Rendering
description: Optimizing TUI rendering performance through buffer diffing, partial redraws, frame rate control, and minimizing escape sequence output.
---

# Performance Rendering

> **What you'll learn:**
> - How Ratatui's buffer diff algorithm minimizes the bytes written to the terminal by only updating changed cells
> - Frame rate control strategies: fixed tick rate, event-driven rendering, and adaptive frame rates based on content changes
> - Profiling TUI rendering performance to identify bottlenecks in layout calculation, widget rendering, and terminal I/O

Performance matters more in TUI applications than many developers expect. A terminal emulator processes every byte you write -- escape sequences for cursor movement, style changes, and character output all flow through the PTY, the terminal emulator's parser, and the font renderer. Writing too much output per frame causes visible lag, flickering, or dropped frames, especially over SSH or in multiplexed sessions. Understanding where performance bottlenecks occur and how to address them is essential for building a smooth agent interface.

## The Cost of Terminal I/O

Every cell update requires multiple bytes of terminal output. Consider what it takes to change a single cell at position (10, 5) from a white space to a red 'X':

```rust
fn demonstrate_io_cost() {
    // To update ONE cell, Ratatui writes approximately:
    // 1. Cursor position: \x1b[5;10H           = 8 bytes
    // 2. Set foreground:  \x1b[38;2;255;0;0m   = 17 bytes
    // 3. Set background:  \x1b[48;2;30;30;46m  = 17 bytes
    // 4. The character:   X                      = 1 byte
    // Total: ~43 bytes for one cell

    // A full 80x24 screen has 1,920 cells
    // Worst case (every cell changed): ~82,560 bytes per frame
    // At 30 fps: ~2.4 MB/s of terminal output

    // This is why diffing matters:
    // If only 50 cells changed: ~2,150 bytes per frame
    // At 30 fps: ~64 KB/s -- a 40x reduction

    let full_redraw_bytes = 1920 * 43;
    let diff_redraw_bytes = 50 * 43;
    let ratio = full_redraw_bytes as f64 / diff_redraw_bytes as f64;

    println!("Full redraw: ~{} bytes/frame", full_redraw_bytes);
    println!("Diff redraw (50 cells): ~{} bytes/frame", diff_redraw_bytes);
    println!("Reduction factor: {:.0}x", ratio);
}

fn main() {
    demonstrate_io_cost();
}
```

## How Ratatui's Buffer Diff Works

Ratatui's double-buffering strategy is the primary performance optimization. Let's look at how the diff algorithm minimizes output:

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

fn demonstrate_diff_optimization() {
    let area = Rect::new(0, 0, 40, 10);

    // Previous frame
    let mut prev = Buffer::empty(area);
    for y in 0..10 {
        for x in 0..40 {
            prev.set_string(
                x, y, " ",
                Style::default().bg(Color::Black),
            );
        }
    }
    prev.set_string(5, 3, "Hello, World!", Style::default().fg(Color::White));
    prev.set_string(5, 5, "Status: OK", Style::default().fg(Color::Green));

    // Current frame (only status changed)
    let mut curr = Buffer::empty(area);
    for y in 0..10 {
        for x in 0..40 {
            curr.set_string(
                x, y, " ",
                Style::default().bg(Color::Black),
            );
        }
    }
    curr.set_string(5, 3, "Hello, World!", Style::default().fg(Color::White));
    curr.set_string(5, 5, "Status: ER", Style::default().fg(Color::Red));

    // Compute the diff
    let diff = prev.diff(&curr);

    println!("Total cells: {}", area.width as usize * area.height as usize);
    println!("Changed cells: {}", diff.len());

    // The diff contains only the cells that differ
    for (x, y, cell) in &diff {
        println!("  Changed at ({}, {}): '{}'", x, y, cell.symbol());
    }
}

fn main() {
    demonstrate_diff_optimization();
}
```

The diff algorithm also optimizes the cursor movement between changed cells. Instead of positioning the cursor for every changed cell, Ratatui detects consecutive changes on the same line and writes them in sequence, avoiding redundant cursor-move sequences.

::: python Coming from Python
Python's Rich uses a similar strategy with its `Live` display. Rich computes the difference between the previous and current rendered output and writes only the changed portions. Textual goes further with its "dirty widget" tracking -- it knows which widgets changed and only re-renders those subtrees. Ratatui's approach is simpler (full-frame diff at the cell level) but effective because the diff is very fast compared to the I/O cost.
:::

## Frame Rate Control

Not every frame needs to be rendered. The three main frame rate strategies are:

### Event-Driven Rendering

Only redraw when something changes. This is the most efficient approach for idle periods:

```rust
use crossterm::event::{self, Event};
use std::time::Duration;

struct App {
    dirty: bool,
    should_quit: bool,
}

fn event_driven_loop(app: &mut App) {
    // Only render when app.dirty is true
    // Events set dirty = true when they change state

    // Pseudocode for the loop:
    // loop {
    //     if app.dirty {
    //         terminal.draw(|f| view(f, app))?;
    //         app.dirty = false;
    //     }
    //
    //     // Block until an event arrives (no CPU usage while idle)
    //     let event = event::read()?;
    //     handle_event(app, event); // Sets app.dirty = true if state changed
    //
    //     if app.should_quit { break; }
    // }

    println!("Event-driven: zero CPU when idle, instant response to input.");
    println!("Best for: simple UIs that only change on user input.");
}

fn main() {
    let mut app = App { dirty: true, should_quit: false };
    event_driven_loop(&mut app);
}
```

### Fixed Tick Rate

Redraw at a fixed interval, checking for events between frames. This is needed when the UI has animations or streaming content:

```rust
use std::time::{Duration, Instant};

struct FrameTimer {
    target_fps: u32,
    frame_duration: Duration,
    last_frame: Instant,
    frame_count: u64,
    total_frame_time: Duration,
}

impl FrameTimer {
    fn new(target_fps: u32) -> Self {
        Self {
            target_fps,
            frame_duration: Duration::from_secs(1) / target_fps,
            last_frame: Instant::now(),
            frame_count: 0,
            total_frame_time: Duration::ZERO,
        }
    }

    /// Returns how long to poll for events before the next frame is due
    fn time_until_next_frame(&self) -> Duration {
        let elapsed = self.last_frame.elapsed();
        self.frame_duration.saturating_sub(elapsed)
    }

    /// Record that a frame was rendered
    fn frame_rendered(&mut self) {
        let now = Instant::now();
        let frame_time = now - self.last_frame;
        self.total_frame_time += frame_time;
        self.frame_count += 1;
        self.last_frame = now;
    }

    /// Get the average frame time
    fn avg_frame_time_ms(&self) -> f64 {
        if self.frame_count == 0 {
            return 0.0;
        }
        self.total_frame_time.as_secs_f64() * 1000.0 / self.frame_count as f64
    }

    /// Get the effective FPS
    fn effective_fps(&self) -> f64 {
        if self.avg_frame_time_ms() == 0.0 {
            return 0.0;
        }
        1000.0 / self.avg_frame_time_ms()
    }
}

fn main() {
    let mut timer = FrameTimer::new(30); // Target 30 FPS

    println!("Fixed tick rate loop:");
    println!("  Target FPS: {}", timer.target_fps);
    println!("  Frame budget: {:.1}ms", timer.frame_duration.as_secs_f64() * 1000.0);
    println!();
    println!("  loop {{");
    println!("      let timeout = timer.time_until_next_frame();");
    println!("      if event::poll(timeout)? {{");
    println!("          handle_event(event::read()?);");
    println!("      }}");
    println!("      terminal.draw(|f| view(f, &app))?;");
    println!("      timer.frame_rendered();");
    println!("  }}");

    // Simulate some frames
    for _ in 0..5 {
        std::thread::sleep(Duration::from_millis(16));
        timer.frame_rendered();
    }
    println!("\nAfter 5 frames:");
    println!("  Avg frame time: {:.1}ms", timer.avg_frame_time_ms());
    println!("  Effective FPS: {:.1}", timer.effective_fps());
}
```

### Adaptive Frame Rate

The best approach for a coding agent combines both strategies: event-driven when idle, higher frame rate during streaming:

```rust
use std::time::{Duration, Instant};

enum RenderMode {
    /// No content is changing. Only redraw on user input.
    Idle,
    /// LLM is streaming tokens. Redraw at a moderate rate.
    Streaming,
    /// Animation playing (e.g., spinner). Redraw at high rate.
    Animating,
}

struct AdaptiveFrameRate {
    mode: RenderMode,
    last_render: Instant,
}

impl AdaptiveFrameRate {
    fn new() -> Self {
        Self {
            mode: RenderMode::Idle,
            last_render: Instant::now(),
        }
    }

    /// Get the poll timeout based on current mode
    fn poll_timeout(&self) -> Duration {
        match self.mode {
            RenderMode::Idle => Duration::from_secs(60), // Wake only on events
            RenderMode::Streaming => Duration::from_millis(50), // ~20 FPS
            RenderMode::Animating => Duration::from_millis(16), // ~60 FPS
        }
    }

    /// Check if enough time has passed for a redraw
    fn should_render(&self) -> bool {
        let min_interval = match self.mode {
            RenderMode::Idle => Duration::from_millis(100),
            RenderMode::Streaming => Duration::from_millis(50),
            RenderMode::Animating => Duration::from_millis(16),
        };
        self.last_render.elapsed() >= min_interval
    }

    fn set_mode(&mut self, mode: RenderMode) {
        self.mode = mode;
    }

    fn rendered(&mut self) {
        self.last_render = Instant::now();
    }
}

fn main() {
    let mut fps = AdaptiveFrameRate::new();

    println!("Adaptive frame rate:");
    println!("  Idle mode:      ~0 FPS (event-driven only)");
    println!("  Streaming mode: ~20 FPS (smooth token display)");
    println!("  Animating mode: ~60 FPS (spinners, transitions)");
    println!();

    fps.set_mode(RenderMode::Streaming);
    println!("Switched to streaming mode.");
    println!("Poll timeout: {:?}", fps.poll_timeout());
    println!("Should render: {}", fps.should_render());
}
```

## Minimizing Style Changes

Each style change generates escape sequences. Optimizing the order of cell writes to minimize style switches can reduce output significantly:

```rust
fn demonstrate_style_optimization() {
    // Naive approach: write cells left-to-right, top-to-bottom
    // If adjacent cells have different styles, each transition costs ~30 bytes

    // Example: alternating red and blue cells on one line
    // Naive: \x1b[31mR\x1b[34mB\x1b[31mR\x1b[34mB... (30+ bytes per cell)
    // That is 10 style changes for 10 cells

    // Ratatui's optimization:
    // The diff algorithm groups consecutive cells with the same style
    // and writes them in a single run, minimizing style-change sequences

    // You can also help by designing your UI to minimize style boundaries:
    // - Use consistent background colors for large regions
    // - Group same-styled content together
    // - Avoid per-character style changes when possible

    println!("Style change optimization tips:");
    println!("  1. Use consistent backgrounds for large areas");
    println!("  2. Group same-styled content on the same lines");
    println!("  3. Prefer fewer, larger styled regions over many small ones");
    println!("  4. Ratatui's diff groups consecutive same-style cells automatically");
}

fn main() {
    demonstrate_style_optimization();
}
```

## Buffered I/O

Ratatui writes through a `BufWriter` by default, but ensuring your backend uses buffered I/O is important for performance:

```rust
use std::io::{self, BufWriter, Write};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

fn create_buffered_terminal() -> io::Result<Terminal<CrosstermBackend<BufWriter<io::Stdout>>>> {
    // Wrapping stdout in BufWriter ensures that escape sequences
    // are batched into fewer write() system calls.
    // Each write() syscall has overhead (~microseconds),
    // so reducing the number of calls improves performance.

    let stdout = io::stdout();
    let buffered = BufWriter::with_capacity(
        8192, // 8 KB buffer -- enough for most frames
        stdout,
    );
    let backend = CrosstermBackend::new(buffered);
    Terminal::new(backend)
}

fn main() {
    match create_buffered_terminal() {
        Ok(_) => println!("Buffered terminal created successfully."),
        Err(e) => println!("Error: {}", e),
    }

    println!();
    println!("BufWriter reduces write() syscalls:");
    println!("  Without: each escape sequence = 1 syscall");
    println!("  With 8KB buffer: entire frame = 1-2 syscalls");
    println!("  This matters especially over SSH/network connections.");
}
```

::: wild In the Wild
Claude Code and other production terminal tools are particularly sensitive to rendering performance when running over SSH. The network round-trip between the remote server and the local terminal emulator adds latency to every byte of output. Minimizing output volume through efficient diffing, buffered I/O, and adaptive frame rates is the difference between a smooth experience and a laggy, unusable one. Production agents typically reduce their rendering rate over SSH and batch updates more aggressively.
:::

## Profiling Rendering Performance

When your TUI feels slow, you need to identify the bottleneck. Instrument the rendering pipeline to measure each phase:

```rust
use std::time::{Duration, Instant};

struct RenderProfile {
    event_handling: Duration,
    state_update: Duration,
    layout_calc: Duration,
    widget_render: Duration,
    buffer_diff: Duration,
    terminal_write: Duration,
    total: Duration,
}

impl RenderProfile {
    fn report(&self) {
        let total_ms = self.total.as_secs_f64() * 1000.0;
        println!("Frame profile ({:.2}ms total):", total_ms);
        println!("  Event handling: {:.2}ms ({:.0}%)",
            self.event_handling.as_secs_f64() * 1000.0,
            self.event_handling.as_secs_f64() / self.total.as_secs_f64() * 100.0);
        println!("  State update:   {:.2}ms ({:.0}%)",
            self.state_update.as_secs_f64() * 1000.0,
            self.state_update.as_secs_f64() / self.total.as_secs_f64() * 100.0);
        println!("  Layout calc:    {:.2}ms ({:.0}%)",
            self.layout_calc.as_secs_f64() * 1000.0,
            self.layout_calc.as_secs_f64() / self.total.as_secs_f64() * 100.0);
        println!("  Widget render:  {:.2}ms ({:.0}%)",
            self.widget_render.as_secs_f64() * 1000.0,
            self.widget_render.as_secs_f64() / self.total.as_secs_f64() * 100.0);
        println!("  Buffer diff:    {:.2}ms ({:.0}%)",
            self.buffer_diff.as_secs_f64() * 1000.0,
            self.buffer_diff.as_secs_f64() / self.total.as_secs_f64() * 100.0);
        println!("  Terminal write:  {:.2}ms ({:.0}%)",
            self.terminal_write.as_secs_f64() * 1000.0,
            self.terminal_write.as_secs_f64() / self.total.as_secs_f64() * 100.0);
    }
}

fn simulate_profile() -> RenderProfile {
    // Simulate typical frame timings
    RenderProfile {
        event_handling: Duration::from_micros(50),
        state_update: Duration::from_micros(100),
        layout_calc: Duration::from_micros(200),
        widget_render: Duration::from_micros(800),
        buffer_diff: Duration::from_micros(150),
        terminal_write: Duration::from_millis(2),
        total: Duration::from_micros(3300),
    }
}

fn main() {
    let profile = simulate_profile();
    profile.report();

    println!();
    println!("Common bottlenecks:");
    println!("  1. Terminal write (I/O bound) -- reduce output volume");
    println!("  2. Widget render (CPU bound) -- cache expensive computations");
    println!("  3. Layout calc -- layout is cached, but complex nested layouts add up");
    println!("  4. Syntax highlighting -- cache highlighted output across frames");
}
```

## Key Takeaways

- Ratatui's buffer diffing is the core performance optimization -- it compares the current and previous frame buffers cell-by-cell and writes only changed cells, reducing terminal I/O by 10-40x in typical scenarios.
- Frame rate control should adapt to content: event-driven rendering when idle (zero CPU cost), moderate frame rates (~20 FPS) during streaming, and higher rates (~60 FPS) only for animations.
- Terminal I/O is typically the largest bottleneck, especially over SSH. Use `BufWriter` to batch writes, minimize style changes between adjacent cells, and reduce output volume.
- Profile your rendering pipeline by timing each phase (event handling, state update, layout, widget render, buffer diff, terminal write) to identify where the time is actually spent.
- Cache expensive computations (syntax highlighting, markdown parsing) across frames rather than recomputing them on every render call, since most frames show the same or similar content.
