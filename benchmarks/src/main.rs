//extern crate clap;
use std::{
    collections::HashMap,
    fmt,
    fs::{create_dir_all, File},
    io::Write,
    sync::mpsc,
    thread,
};

mod acc;
mod algo;
mod exp;

use acc::Accumulator;
use algo::{FlipHash64, FlipHashXXH3128, FlipHashXXH364, JumpHash};
use clap::Parser;
use exp::{Collisions, Experiment, IndependenceAcrossRanges, IndependenceAcrossSeeds, Regularity};
use flip_hash_benchmarks::jump_hash;
use itertools::Itertools;

const RESULT_DIR: &str = "results";
const DEFAULT_ALGORITHMS: [Algorithm; 4] = [
    Algorithm::FlipHash64,
    Algorithm::FlipHashXXH364,
    Algorithm::FlipHashXXH3128,
    Algorithm::JumpHash,
];

#[derive(Parser, Debug)]
enum Command {
    /// Tests the uniformity of the distribution of hashes using a chi-squared
    /// test.
    Regularity {
        #[clap(short, long)]
        range_end: u64,
        #[clap(short, long)]
        input_size_bytes: usize,
        #[clap(short, long, default_values_t=DEFAULT_ALGORITHMS)]
        algorithms: Vec<Algorithm>,
    },

    /// Compares the number of collisions with the expected value if the
    /// distribution is uniform. The number of collisions is related to the L2
    /// distance to the uniform distribution, so this is another way to test for
    /// regularity.
    Collisions {
        #[clap(short, long)]
        range_end: u64,
        #[clap(short, long)]
        input_size_bytes: usize,
        #[clap(short, long, default_values_t=DEFAULT_ALGORITHMS)]
        algorithms: Vec<Algorithm>,
    },

    /// Tests the mutual independence across a given number of ranges, given
    /// that hashes are pairwise distinct, using a chi-squared test.
    IndependenceAcrossRanges {
        #[clap(short, long)]
        range_end: Vec<u64>,
        #[clap(short, long)]
        input_size_bytes: usize,
        #[clap(short, long, default_values_t=DEFAULT_ALGORITHMS)]
        algorithms: Vec<Algorithm>,
    },

    /// Tests the mutual independence acros seeds using a chi-squared test.
    IndependenceAcrossSeeds {
        #[clap(short, long)]
        range_end: u64,
        #[clap(short, long)]
        num_seeds: usize,
        #[clap(short, long)]
        input_size_bytes: usize,
        #[clap(short, long, default_values_t=DEFAULT_ALGORITHMS)]
        algorithms: Vec<Algorithm>,
    },
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum Algorithm {
    FlipHash64,
    FlipHashXXH364,
    FlipHashXXH3128,
    JumpHash,
}

impl fmt::Display for Algorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Algorithm::FlipHash64 => "flip-hash64",
                Algorithm::FlipHashXXH364 => "flip-hash-xxh364",
                Algorithm::FlipHashXXH3128 => "flip-hash-xxh3128",
                Algorithm::JumpHash => "jump-hash",
            }
        )
    }
}

fn main() {
    match Command::parse() {
        Command::Regularity {
            range_end,
            input_size_bytes,
            algorithms,
        } => {
            let output_dir = format!("{RESULT_DIR}/regularity");
            create_dir_all(&output_dir).unwrap();
            let mut output = File::create(format!(
                "{output_dir}/{input_size_bytes}_bytes_to_range_to_incl_{range_end}"
            ))
            .unwrap();
            let experiment = Regularity::new(..=range_end, input_size_bytes);
            run_experiment(&mut output, experiment, algorithms);
        }
        Command::Collisions {
            range_end,
            input_size_bytes,
            algorithms,
        } => {
            let output_dir = format!("{RESULT_DIR}/collisions");
            create_dir_all(&output_dir).unwrap();
            let mut output = File::create(format!(
                "{output_dir}/{input_size_bytes}_bytes_to_range_to_incl_{range_end}"
            ))
            .unwrap();
            let experiment = Collisions::new(..=range_end, input_size_bytes);
            run_experiment(&mut output, experiment, algorithms);
        }
        Command::IndependenceAcrossRanges {
            range_end,
            input_size_bytes,
            algorithms,
        } => {
            let output_dir = format!("{RESULT_DIR}/independence_across_ranges");
            create_dir_all(&output_dir).unwrap();
            let mut output = File::create(format!(
                "{output_dir}/{}_bytes_to_ranges_to_incl_{}",
                input_size_bytes,
                range_end.iter().join("_")
            ))
            .unwrap();
            let experiment = IndependenceAcrossRanges::new(
                range_end
                    .iter()
                    .sorted()
                    .map(|&end| ..=end)
                    .collect::<Vec<_>>(),
                input_size_bytes,
            );
            run_experiment(&mut output, experiment, algorithms)
        }
        Command::IndependenceAcrossSeeds {
            range_end,
            num_seeds,
            input_size_bytes,
            algorithms,
        } => {
            let output_dir = format!("{RESULT_DIR}/independence_across_seeds");
            create_dir_all(&output_dir).unwrap();
            let mut output = File::create(format!(
                "{output_dir}/{}_bytes_{}_seeds_to_range_to_incl_{}",
                input_size_bytes, num_seeds, range_end
            ))
            .unwrap();
            let experiment =
                IndependenceAcrossSeeds::new(..=range_end, num_seeds, input_size_bytes);
            run_experiment(&mut output, experiment, algorithms)
        }
    }
}

fn run_experiment<E>(output: &mut impl Write, experiment: E, algorithms: Vec<Algorithm>)
where
    E: Experiment + Clone + Send + 'static,
    <E as Experiment>::Accumulator: Send,
{
    const STEP_SIZE: u64 = 10_000_000;

    assert!(!algorithms.is_empty());

    let (tx, rx) = mpsc::channel();
    for _ in 0..usize::from(thread::available_parallelism().unwrap()) - 1 {
        let thread_tx = tx.clone();
        let thread_experiment = experiment.clone();
        let thread_algorithms = algorithms.clone();
        thread::spawn(move || loop {
            for algorithm in &thread_algorithms {
                match algorithm {
                    Algorithm::FlipHash64 => {
                        thread_tx
                            .send((
                                format!("{}", FlipHash64),
                                thread_experiment.accumulate(&FlipHash64, STEP_SIZE),
                            ))
                            .unwrap();
                    }
                    Algorithm::FlipHashXXH364 => {
                        thread_tx
                            .send((
                                format!("{}", FlipHashXXH364),
                                thread_experiment.accumulate(&FlipHashXXH364, STEP_SIZE),
                            ))
                            .unwrap();
                    }
                    Algorithm::FlipHashXXH3128 => {
                        thread_tx
                            .send((
                                format!("{}", FlipHashXXH3128),
                                thread_experiment.accumulate(&FlipHashXXH3128, STEP_SIZE),
                            ))
                            .unwrap();
                    }
                    Algorithm::JumpHash => {
                        thread_tx
                            .send((
                                format!("{}", JumpHash),
                                thread_experiment.accumulate(&JumpHash, STEP_SIZE),
                            ))
                            .unwrap();
                    }
                }
            }
        });
    }

    let mut accumulators = HashMap::new();
    for (algo, step_accumulator) in rx {
        let algo_accumulator = accumulators
            .entry(algo.clone())
            .or_insert_with(|| experiment.new_accumulator());
        algo_accumulator.merge(&step_accumulator);

        output
            .write_fmt(format_args!("{{\"algo\": \"{algo}\""))
            .unwrap();
        experiment.write_summary(output, algo_accumulator).unwrap();
        output.write_fmt(format_args!("}}\n")).unwrap();
        output.flush().unwrap();
        println!(
            "Processed {:e} keys for {}",
            algo_accumulator.num_iterations(),
            algo
        );
    }
}
