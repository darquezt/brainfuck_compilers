use std::fs;
use std::env;
use std::io::Write;

use brainfuck_compilers::{ parse, Inst };

const BOILERPLATE: &str = include_str!("llvm_ir_boilerplate.ll");

const ASM: [&str; 11] = [
    // > // 0
    "
    %intptr.{{INTPTR+1}} = add i64 %intptr.{{INTPTR}}, {{N}}
    ",
    // < // 1
    "
    %intptr.{{INTPTR+1}} = sub i64 %intptr.{{INTPTR}}, {{N}}
    ",
    // + // 2
    "
    %ptr.{{PTR}} = inttoptr i64 %intptr.{{INTPTR}} to i8*
    %byte.{{BYTE}} = load i8, i8* %ptr.{{PTR}}
    %byte.{{BYTE+1}} = add i8 %byte.{{BYTE}}, {{N}}
    store i8 %byte.{{BYTE+1}}, i8* %ptr.{{PTR}}
    ",
    // - // 3
    "
    %ptr.{{PTR}} = inttoptr i64 %intptr.{{INTPTR}} to i8*
    %byte.{{BYTE}} = load i8, i8* %ptr.{{PTR}}
    %byte.{{BYTE+1}} = sub i8 %byte.{{BYTE}}, {{N}}
    store i8 %byte.{{BYTE+1}}, i8* %ptr.{{PTR}}
    ",
    // before , // 4
    "
    %ptr.{{PTR}} = inttoptr i64 %intptr.{{INTPTR}} to i8*",
    // on multiple , // 5
    "
    call i8 @getchar()
    ",
    // , // 6
    "
    %char.{{CHAR}} = call i8 @getchar()
    %bool.{{BOOL}} = icmp eq i8 255, %char.{{CHAR}}
    %char.{{CHAR+1}} = select i1 %bool.{{BOOL}}, i8 0, i8 %char.{{CHAR}}
    store i8 %char.{{CHAR+1}}, i8* %ptr.{{PTR}}
    ",
    // . // 7
    "
    %ptr.{{PTR}} = inttoptr i64 %intptr.{{INTPTR}} to i8*
    %char.{{CHAR}} = load i8, i8* %ptr.{{PTR}}
",
    // on multiple . // 8
    "    call i8 @putchar(i8 %char.{{CHAR}})
",
    // [ // 9
    "
    %ptr.{{PTR}} = inttoptr i64 %intptr.{{INTPTR}} to i8*
    %byte.{{BYTE}} = load i8, i8* %ptr.{{PTR}}
    %bool.{{BOOL}} = icmp eq i8 0, %byte.{{BYTE}}
    br i1 %bool.{{BOOL}}, label %loop_end_{{END}}, label %loop_start_{{START}}
    loop_start_{{START}}:
    ",
    // ] // 10
    "
    %ptr.{{PTR}} = inttoptr i64 %intptr.{{INTPTR}} to i8*
    %byte.{{BYTE}} = load i8, i8* %ptr.{{PTR}}
    %bool.{{BOOL}} = icmp ne i8 0, %byte.{{BYTE}}
    br i1 %bool.{{BOOL}}, label %loop_start_{{START}}, label %loop_end_{{END}}
    loop_end_{{END}}:
    ",
];

#[derive(Default)]
struct Context {
    intptr: usize,
    ptr: usize,
    byte: usize,
    char: usize,
    bool: usize,
}

fn inst_to_asm(idx: usize, inst: &Inst, ctx: &mut Context) -> String {
    match inst {
        Inst::IncPtr(n) => {
            let llvm = ASM[0]
                .replace("{{N}}", &n.to_string())
                .replace("{{INTPTR}}", &ctx.intptr.to_string())
                .replace("{{INTPTR+1}}", &(ctx.intptr+1).to_string());
            ctx.intptr += 1;
            llvm
        },
        Inst::DecPtr(n) => {
            let llvm = ASM[1]
                .replace("{{N}}", &n.to_string())
                .replace("{{INTPTR}}", &ctx.intptr.to_string())
                .replace("{{INTPTR+1}}", &(ctx.intptr+1).to_string());
            ctx.intptr += 1;
            llvm
        },
        Inst::IncByte(n) => {
            let llvm = ASM[2]
                .replace("{{N}}", &n.to_string())
                .replace("{{PTR}}", &ctx.ptr.to_string())
                .replace("{{INTPTR}}", &ctx.intptr.to_string())
                .replace("{{BYTE}}", &ctx.byte.to_string())
                .replace("{{BYTE+1}}", &(ctx.byte+1).to_string());
            ctx.ptr += 1;
            ctx.byte += 2;
            llvm
        },
        Inst::DecByte(n) => {
            let llvm = ASM[3]
                .replace("{{N}}", &n.to_string())
                .replace("{{PTR}}", &ctx.ptr.to_string())
                .replace("{{INTPTR}}", &ctx.intptr.to_string())
                .replace("{{BYTE}}", &ctx.byte.to_string())
                .replace("{{BYTE+1}}", &(ctx.byte+1).to_string());
            ctx.ptr += 1;
            ctx.byte += 2;
            llvm
        },
        Inst::ReadByte(n) => {
            let mut llvm = ASM[4]
                .replace("{{INTPTR}}", &ctx.intptr.to_string())
                .replace("{{PTR}}", &ctx.ptr.to_string());
            for _ in 0..n-1 {
                llvm.push_str(ASM[5]);
            }
            llvm.push_str(&ASM[6]
                .replace("{{CHAR}}", &ctx.char.to_string())
                .replace("{{CHAR+1}}", &(ctx.char+1).to_string())
                .replace("{{PTR}}", &ctx.ptr.to_string())
                .replace("{{BOOL}}", &ctx.bool.to_string())
            );
            ctx.ptr += 1;
            ctx.bool += 1;
            ctx.char += 2;
            llvm
        },
        Inst::WriteByte(n) => {
            let mut llvm = ASM[7]
                .replace("{{INTPTR}}", &ctx.intptr.to_string())
                .replace("{{PTR}}", &ctx.ptr.to_string())
                .replace("{{CHAR}}", &ctx.char.to_string());
            let write_byte = ASM[8].replace("{{CHAR}}", &ctx.char.to_string());
            for _ in 0..*n {
                llvm.push_str(&write_byte);
            }
            ctx.ptr += 1;
            ctx.char += 1;
            llvm
        },
        Inst::LoopStart(_, goto) => {
            let llvm = ASM[9]
                .replace("{{INTPTR}}", &ctx.intptr.to_string())
                .replace("{{PTR}}", &ctx.ptr.to_string())
                .replace("{{BYTE}}", &ctx.byte.to_string())
                .replace("{{BOOL}}", &ctx.bool.to_string())
                .replace("{{START}}", &idx.to_string())
                .replace("{{END}}", &(goto-1).to_string());
            ctx.ptr += 1;
            ctx.byte += 1;
            ctx.bool += 1;
            llvm
        },
        Inst::LoopEnd(_, goto) => {
            let llvm = ASM[10]
                .replace("{{INTPTR}}", &ctx.intptr.to_string())
                .replace("{{PTR}}", &ctx.ptr.to_string())
                .replace("{{BYTE}}", &ctx.byte.to_string())
                .replace("{{BOOL}}", &ctx.bool.to_string())
                .replace("{{END}}", &idx.to_string())
                .replace("{{START}}", &(goto-1).to_string());
            ctx.ptr += 1;
            ctx.byte += 1;
            ctx.bool += 1;
            llvm
        },
    }
}

fn write_inst_to_asm<W: Write>(instructions: &[Inst], output: &mut W) -> Result<(), Box<dyn std::error::Error>> {
    let mut ctx = Context::default();
    for (idx, inst) in instructions.iter().enumerate() {
        output.write(inst_to_asm(idx, inst, &mut ctx).as_bytes())?;
    }
    
    Ok(())
}

fn parse_and_compile<W: Write>(src: &str, mut output: &mut W) -> Result<(), Box<dyn std::error::Error>> {
    let instructions = parse(src)?;
    
    let mut split_bp = BOILERPLATE.split("{{REPLACE}}");
    let header_bp = split_bp.next().unwrap();
    let footer_bp = split_bp.next().unwrap();
    
    output.write(header_bp.as_bytes())?;
    write_inst_to_asm(&instructions, &mut output)?;
    output.write(footer_bp.as_bytes())?;
    
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        println!("USEAGE: program path/to/brainfuck.b path/to/output.ll");
        std::process::exit(1);
    }

    let src = fs::read_to_string(&args[1])?;
    let mut output = fs::File::create(&args[2])?;

    parse_and_compile(&src, &mut output)?;
    output.flush()?;

    Ok(())
}