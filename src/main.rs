use core::error;
use std::env;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use syscalls::{self, SyscallArgs, Sysno, syscall, syscall_args};

#[derive(Debug, EnumIter)]
enum CastArg {
    // Address of the string.
    String(usize),

    // Also works for negative numbers since it can just be bitwise reinterpreted.
    U64(u64),
}

impl CastArg {
    /// Converts an argument string like "s:hello world" into a CastArg enum.
    fn new(arg: &str) -> Result<Self, Box<dyn error::Error>> {
        // At least "t:v"
        // type, separator and value.
        if arg.len() < 3 {
            return Err(format!("Cannot process arg '{}' as it is too short.", arg).into());
        }
        if !Self::is_type_hint(arg) {
            return Err(format!("Type hint for arg '{}' is unknown!", arg).into());
        }

        // Manually doing it does not feel nice, there has to be a way to do it dynamically.
        // Something like doing a struct for each variants in the enum.

        if let Some(first_char) = arg.chars().nth(0) {
            // "t:v"
            //  ^^
            //  We get those out.
            let owned = Box::leak(arg[2..].to_string().into_boxed_str());

            // Get the pointer to the string.
            if first_char == 's' {
                Ok(Self::String(owned.as_ptr() as usize))

                // Convert to usize.
            } else if first_char == 'n' {
                let n = owned
                    .parse::<usize>()
                    .map_err(|e| format!("Failed to parse arg '{}': {}", owned, e))?;
                Ok(Self::U64(n as u64))
            } else {
                Err(format!("Failed to get first character on argument '{}'!", arg).into())
            }
        } else {
            Err("There is a problem, what problem? I don't know.".into())
        }
    }

    /// Returns the `usize` value from the variant.
    fn get_usize(&self) -> usize {
        match self {
            CastArg::String(a) => *a,
            CastArg::U64(a) => *a as usize,
        }
    }

    /// Returns the string value of the type-hint.
    fn get_type_hint_str(&self) -> &'static str {
        match self {
            CastArg::String(_) => "s",
            CastArg::U64(_) => "n",
        }
    }

    /// Returns true if the input (into lowercase) is a type-hint.
    fn is_type_hint(arg: &str) -> bool {
        let first: char = arg.chars().nth(0).unwrap();
        let first = first.to_lowercase().to_string();

        Self::iter().any(|x| x.get_type_hint_str().to_lowercase() == first)
    }
}

/// Parses input args and return the system call number and its usize arguments.
fn parse_args() -> Result<(Sysno, SyscallArgs), Box<dyn error::Error>> {
    let args: Vec<String> = env::args().collect();
    dbg!(&args);

    // Self + syscall number
    if args.len() < 2 {
        return Err("Not enough arguments!".into());
    }

    // Max 6 arguments.
    let mut cast_args: Vec<CastArg> = Vec::with_capacity(6);
    let sysno_num = args[1].parse::<usize>().map_err(|e| {
        format!(
            "Failed to parse syscall number from '{}'. Error: {}",
            args[1], e
        )
    })?;
    let sysno = Sysno::new(sysno_num).ok_or(format!("System call '{}' unknown!", sysno_num))?;

    // Args except [0] and [1].
    for (idx, arg) in args[2..].iter().enumerate() {
        let tokens: Vec<&str> = arg.splitn(2, ':').collect();

        if tokens.len() != 2 {
            return Err(format!("Argument number {} is missing a type-hint.", idx + 1).into());
        }

        cast_args.push(CastArg::new(arg)?);
    }

    let mut usize_args: Vec<usize> = cast_args.iter().map(|x| x.get_usize()).collect();
    // Only keep the 6 items at max, else the program will panic because of the below code.
    usize_args.truncate(6);

    let res = match usize_args.len() {
        0 => syscall_args!(),
        1 => syscall_args!(usize_args[0]),
        2 => syscall_args!(usize_args[0], usize_args[1]),
        3 => syscall_args!(usize_args[0], usize_args[1], usize_args[2]),
        4 => syscall_args!(usize_args[0], usize_args[1], usize_args[2], usize_args[3]),
        5 => syscall_args!(
            usize_args[0],
            usize_args[1],
            usize_args[2],
            usize_args[3],
            usize_args[4]
        ),
        6 => syscall_args!(
            usize_args[0],
            usize_args[1],
            usize_args[2],
            usize_args[3],
            usize_args[4],
            usize_args[5]
        ),
        _ => panic!("Too many arguments"),
    };

    Ok((sysno, res))
}

fn main() -> Result<(), Box<dyn error::Error>> {
    match parse_args() {
        Ok(args) => {
            unsafe {
                match syscall(args.0, &args.1) {
                    Ok(code) => println!("Syscall sucessfully executed: {code}."),
                    Err(e) => eprintln!("Failed to execute syscall: {e}"),
                }
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}
