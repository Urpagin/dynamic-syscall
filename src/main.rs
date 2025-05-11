use core::error;
use logos::{Lexer, Logos};
use std::{
    env::{self},
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{self, Path, PathBuf},
};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use syscalls::{self, SyscallArgs, Sysno, syscall, syscall_args};

const COMMENT_STR: &str = "#";

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
    println!("Input arguments: {args:#?}\n\n");

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

/// Returns true if --compile <syslang source file> is provided in the args
fn does_interpret_syslang() -> Option<PathBuf> {
    const ARG_NAME: &str = "--interpret";
    const SYSLANG_EXT: &str = "scx";

    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        return None;
    }

    let arg = args[1].clone();
    let filepath = path::Path::new(&args[2]);

    if arg != ARG_NAME {
        return None;
    }

    if !filepath.exists() {
        eprintln!("--interpret file argument is incorrect!");
        return None;
    }

    if let Some(ext) = filepath.extension() {
        if ext != SYSLANG_EXT {
            eprintln!("--interpret file is not a .scx file!");
            return None;
        }
        Some(filepath.into())
    } else {
        eprintln!("--interpret file is not a .scx file!");
        None
    }
}

#[derive(Logos, Debug, PartialEq)]
#[logos(skip r"[ \t\n\f]+")] // Ignore this regex pattern between tokens
enum Token {
    // Keyword 'syscall'
    #[token("syscall")]
    Syscall,

    // I think this pattern is flawed: it also takes numbers inside strings.
    #[regex(r"[+-]?[\d]+")]
    Number,

    // This regex pattern is supposed to match everything inside double quotes.
    #[regex(r#"\"([^\"]*)\""#)]
    String,
}

/// Returns a string with first and last character removed.
fn parse_string_literal(string: &str) -> String {
    if string.len() < 2 {
        eprintln!("{string}");
        panic!("Failed to slice string to remove quotes. String len is < 2.");
    }
    let s = &string[1..string.len() - 1]; // strip surrounding quotes
    s.replace("\\n", "\n").replace("\\t", "\t")
}

/// Well I mean, this function lexes, interprets, bakes eggs, cuts onions....
fn lex(file: &Path) {
    let file = File::open(file).expect("Failed to open source file");
    let reader = BufReader::new(file);

    let mut calls: Vec<Box<dyn FnOnce()>> = Vec::new();
    for (idx, line_result) in reader.lines().enumerate() {
        let line: String = line_result.expect("Failed to read line in source file");

        // Skip comments (only works on comments having their own lines)
        if line.starts_with(COMMENT_STR) {
            continue;
        }

        let mut lexer = Token::lexer(&line);
        println!("--- LEXING l{:04} ---", idx + 1);
        add_call(&mut calls, &mut lexer, idx + 1);
        println!("\n")
    }

    interpret(calls);
}

/// Actually executes the code from the source files.
fn interpret(calls: Vec<Box<dyn FnOnce()>>) {
    println!("Interpreting...");

    for call in calls.into_iter() {
        call();
    }
}

/// Parses a line from the lexer and adds a call to the calls.
fn add_call(calls: &mut Vec<Box<dyn FnOnce()>>, lexer_line: &mut Lexer<'_, Token>, line: usize) {
    let mut syscall_buffer: Vec<(usize, Token, String)> = Vec::new();
    // This is smelly isn't it?
    let mut is_syscall: bool = false;

    // Index of the tokens in a line.
    // e.g., syscall ... .... Here syscall is 0.
    let mut idx: usize = 0;

    while let Some(t) = lexer_line.next() {
        let token: Token = t.expect("Failed to tokenize/lex");
        let slice = lexer_line.slice();
        println!("Got token: {token:?} with slice: {slice}");

        // I should make a function to delete the syscall parsing?
        if token == Token::Syscall {
            is_syscall = true;
            idx += 1;
            continue;
        }

        match token {
            Token::Syscall => unreachable!(),
            Token::Number => syscall_buffer.push((idx, token, slice.to_string())),
            Token::String => syscall_buffer.push((idx, token, parse_string_literal(slice))),
            _ => {
                panic!("Unexpected token: {:?}", token);
            }
        }

        idx += 1;
    }

    let sc_buf_len: usize = syscall_buffer.len();
    // 7 because 6 args MAX + sysno = 7.
    if is_syscall && (sc_buf_len > 7 || sc_buf_len == 0) {
        panic!("Syscall must have at least 1 and at most 6 arguments.");
    }

    if is_syscall {
        // the name.....
        let mut sc_final_args: Vec<usize> = Vec::with_capacity(6);

        for arg in syscall_buffer {
            let idx: usize = arg.0;
            let token: Token = arg.1;
            let slice: String = arg.2;

            match token {
                Token::Number => {
                    let n: usize = slice.parse().expect("Failed to parse number");
                    sc_final_args.push(n);
                }
                Token::String => {
                    // Very bad right, I'm leaking memory in a loop :skullemoji:
                    // I just want my weird code to work ASAP, to hell best practices!
                    let slice_leak: &'static str = Box::leak(slice.to_string().into_boxed_str());
                    let str_ptr: usize = slice_leak.as_ptr() as usize;
                    sc_final_args.push(str_ptr);
                }
                _ => panic!("Unexpected token: {:?}", token),
            }
        }

        // now we just append the calls vector with the right function.
        // This function that when called, will invoke the syscall using the
        // sc_final_args.

        let sysno = Sysno::new(sc_final_args[0]).expect("Failed to parse syscall number");
        sc_final_args.remove(0);

        let sysargs = match sc_final_args.len() {
            0 => syscall_args!(),
            1 => syscall_args!(sc_final_args[0]),
            2 => syscall_args!(sc_final_args[0], sc_final_args[1]),
            3 => syscall_args!(sc_final_args[0], sc_final_args[1], sc_final_args[2]),
            4 => syscall_args!(
                sc_final_args[0],
                sc_final_args[1],
                sc_final_args[2],
                sc_final_args[3]
            ),
            5 => syscall_args!(
                sc_final_args[0],
                sc_final_args[1],
                sc_final_args[2],
                sc_final_args[3],
                sc_final_args[4]
            ),
            6 => syscall_args!(
                sc_final_args[0],
                sc_final_args[1],
                sc_final_args[2],
                sc_final_args[3],
                sc_final_args[4],
                sc_final_args[5]
            ),
            _ => panic!("Too many arguments"),
        };

        // The code terrifies me

        calls.push(Box::new(move || {
            invoke_syscall_interpret(sysno, sysargs, line)
        }));
    }
}

/// Invokes the syscall immediatly when called using the passed arguments.
fn invoke_syscall_interpret(sysno: Sysno, sysargs: SyscallArgs, line: usize) {
    unsafe {
        match syscall(sysno, &sysargs) {
            Ok(code) => {
                println!("Syscall at line {} returned: {}", line + 1, code);
                //println!("Syscall sucessfully executed.\nSyscall return value: {code}")
            }
            Err(e) => eprintln!("Failed to execute syscall: {e}"),
        }
    }
}

/// Invokes a single syscall from the CLI arguments.
fn begin_arguments() -> Result<(), Box<dyn error::Error>> {
    match parse_args() {
        Ok(args) => {
            unsafe {
                match syscall(args.0, &args.1) {
                    Ok(code) => {
                        println!("Syscall sucessfully executed.\nSyscall return value: {code}")
                    }
                    Err(e) => eprintln!("Failed to execute syscall: {e}"),
                }
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Interprets a syslang file
fn begin_file(filepath: &Path) -> Result<(), Box<dyn error::Error>> {
    println!("Interpreting file {filepath:?}!");
    println!("Begin lexing...");
    lex(filepath);

    Ok(())
}

fn main() -> Result<(), Box<dyn error::Error>> {
    if let Some(filepath) = does_interpret_syslang() {
        begin_file(&filepath)?;
    } else {
        begin_arguments()?;
    }

    Ok(())
}
