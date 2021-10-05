use std::env;
use std::fs::File;
use std::io;
use std::path::Path;
use std::process;

use corken::*;

fn main() -> io::Result<()> {
    let args = env::args_os().collect::<Vec<_>>();
    let input_file = match args.as_slice() {
        [] => unreachable!(),

        [exe] => {
            eprintln!("Corken Payments Engine\n");
            eprintln!("USAGE:\n    {} <input_file>\n", exe.to_string_lossy());

            process::exit(1);
        }

        [_, first, ..] => File::open(Path::new(first))?,
    };

    // NOTE: csv wraps all streams in BufReader.
    let csv_rdr = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(input_file);

    let state = State::from_iterator(csv_rdr.into_deserialize().filter_map(Result::ok));

    let mut writer = csv::WriterBuilder::new().from_writer(io::stdout());

    state
        .accounts()
        .try_for_each(|x| writer.serialize(x))
        .expect("Failed to serialize accounts.");

    Ok(())
}
