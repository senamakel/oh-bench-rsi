//! `ohrsi-swarm` — runs the memory-rsi hive (see `swarm/README.md`).

mod package;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("validate") => {
            let dir = args.get(1).map_or("hives/memory-rsi", String::as_str);
            let lowered = package::load(std::path::Path::new(dir))?;
            println!(
                "{}: {} seats ({}), {} desks",
                lowered.id,
                lowered.seats.len(),
                lowered.seats.iter().map(|s| s.id.as_str()).collect::<Vec<_>>().join(", "),
                lowered.desks.len()
            );
            Ok(())
        }
        _ => anyhow::bail!("usage: ohrsi-swarm validate [hive-dir]"),
    }
}
