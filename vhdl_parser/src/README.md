# rust\_hdl/vhdl\_parser Source Code Overview

This documentation is not kept up to date.
It's purpose is to give a new developer on the codebase a vague idea of what is going on.

# Files

The files can be divided into five categories.
 * Ast
    The files define the structures used to build the abstract syntax tree.
 * Parsing (Not Specific)
    These files are involved in the parsing but are fairly general and aren't responsible for a specific aspect of VHDL.
 * Parsing (Specific)
    Files responsible some specific VHDL structure.
 * Semantic
    Going beyond parsing and starting to make sure it makes sense.
 * Misc

## Ast
 * has\_ident.rs
 * mod.rs
    Defines the AST.
 * name.rs

## Parsing - Not Specific
 * common.rs
     - Little parsing utilities
 * parser.rs
 * project.rs
 * source.rs
     - Represent source files and position therein.
 * symbol\_table.rs
     - Mappings between names and ids.
 * tokenizer.rs
 * token\_stream.rs

## Parsing - Specific
 * attributes.rs
 * compmonent\_declaration.rs
 * concurrent\_statement.rs
 * configuration.rs
 * context.rs
 * declarative\_part.rs
 * design\_unit.rs
 * expression.rs
 * interface\_declaration.rs
 * names.rs
 * object\_declaration.rs
 * range.rs
 * sequential\_statement.rs
 * subprogram.rs
 * subtype\_indication.rs
 * type\_declaration.rs
 * waveform.rs

## Semantic
 * library.rs
    Semantic stuff for dealing with libraries (EntityDesignUnit, PackageDesignUnit, Library, DesignRoot)
 * semantic.rs
    Checking that definitions in different places aren't conflicting.
    This requires semantic analysis.
 * declarative\_region.rs
    AnyDeclaration, VisibleDeclaration, ParentDeclaration, DeclarativeRegion, impl InterfaceDeclaration

## Misc
 * config.rs
 * latin\_1.rs
    Moving between latin and utf8.
 * main.rs
    A little demo.
 * message.rs
    Defines messages emmitted from parser.
 * test\_utils.rs

# Overview

 - Parse VHDL into AST.
 - Semantic analysis.
   - Step through file keeping local symbol tables for each declarative region.
   - Each symbol structure in the AST should point the location in the AST where that symbol is defined or to a
     table that lists all the undefined symbols.
   - Parse all files
   - Semantically analyze all files.
   - Link analyzed files.

 



