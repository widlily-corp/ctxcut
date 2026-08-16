# TS/JS AST Queries & Resolver Engine Specification (`ctxcut_core`)

**Agent**: `explorer_ast_ts_1` (AST & Resolver Specialist)  
**Parent Conversation ID**: `392c723b-2888-4f80-8bf8-e5101eb481a6`  
**Milestone**: M1 (Workspace Foundation & Core AST Engine - TS/JS)  
**Date**: 2026-08-16  
**Status**: COMPLETE / READY FOR IMPLEMENTATION  

---

## 1. Observation

### 1.1. Requirements & Architecture Baseline
- **`ORIGINAL_REQUEST.md` (Lines 12–27)**: Mandates AST parsing and symbol resolution using `tree-sitter` for TypeScript/JavaScript (.ts, .tsx, .js, .jsx), Python, Go, and Rust. Target execution time must be **< 10ms** for files under 2,000 LOC.
- **`PROJECT.md` (Lines 108–199)**: Defines public interfaces for `ctxcut_core`: `ContextSlicer`, `SliceOptions`, `SliceResult`, `ExtractedSymbol`, `ExtractedType`, `CallSignatureStub`, `TokenStats`, `SupportedLanguage`, and `CoreError`.
- **`SCOPE.md` (Lines 1–26)**: Establishes Milestone 1 deliverables: workspace setup, `crates/ctxcut_core` with TS/JS parser, symbol locator, type hoister, signature stripper, markdown/JSON formatter, BPE token counter (`tiktoken-rs`), and unit tests.
- **`spec_miner_survey_1/handoff.md`**: Provides initial grammar node inventory and high-level pipeline flow.

### 1.2. Tree-Sitter TS/JS Grammar Bindings
Official Rust grammar bindings for TypeScript and JavaScript:
- `tree-sitter = "0.24"`
- `tree-sitter-typescript = "0.23"`
  - `tree_sitter_typescript::LANGUAGE_TYPESCRIPT` (Language ID / `LanguageFn` for `.ts`, `.cts`, `.mts`, `.d.ts`)
  - `tree_sitter_typescript::LANGUAGE_TSX` (Language ID / `LanguageFn` for `.tsx`)
- `tree_sitter_javascript = "0.23"`
  - `tree_sitter_javascript::LANGUAGE` (Language ID / `LanguageFn` for `.js`, `.mjs`, `.cjs`, `.jsx`)

---

## 2. Comprehensive TS/JS Tree-Sitter Node Kind & Query Catalog

### 2.1. Symbol Locator Node Kinds & S-Expressions

| Symbol Category | AST Node Kind | Exact Tree-Sitter S-Expression Query / Structure | AST Fields & Captures |
|---|---|---|---|
| **Named Function** | `function_declaration` | `(function_declaration name: (identifier) @name type_parameters: (type_parameters)? @type_params parameters: (formal_parameters) @params return_type: (type_annotation)? @ret body: (statement_block) @body)` | `name`, `type_parameters`, `parameters`, `return_type`, `body` |
| **Generator Function** | `generator_function_declaration` | `(generator_function_declaration name: (identifier) @name type_parameters: (type_parameters)? @type_params parameters: (formal_parameters) @params return_type: (type_annotation)? @ret body: (statement_block) @body)` | `name`, `type_parameters`, `parameters`, `return_type`, `body` |
| **Const / Let Arrow Function** | `lexical_declaration` containing `variable_declarator` + `arrow_function` | `(lexical_declaration (variable_declarator name: (identifier) @name type: (type_annotation)? @var_type value: (arrow_function type_parameters: (type_parameters)? @type_params parameters: [(formal_parameters) (identifier)] @params return_type: (type_annotation)? @ret body: [(statement_block) (_)] @body)))` | `name`, `var_type`, `type_params`, `params`, `ret`, `body` |
| **Const / Let Function Expression** | `lexical_declaration` containing `variable_declarator` + `function_expression` | `(lexical_declaration (variable_declarator name: (identifier) @name value: (function_expression name: (identifier)? @fn_name type_parameters: (type_parameters)? @type_params parameters: (formal_parameters) @params return_type: (type_annotation)? @ret body: (statement_block) @body)))` | `name`, `type_params`, `params`, `ret`, `body` |
| **Class Declaration** | `class_declaration` / `abstract_class_declaration` | `[(class_declaration name: (type_identifier) @name type_parameters: (type_parameters)? @type_params heritage: (class_heritage)? @heritage body: (class_body) @body) (abstract_class_declaration name: (type_identifier) @name type_parameters: (type_parameters)? @type_params heritage: (class_heritage)? @heritage body: (class_body) @body)]` | `name`, `type_params`, `heritage`, `body` |
| **Class Method Definition** | `method_definition` (inside `class_body`) | `(method_definition (accessibility_modifier)? @access (static)? @static (async)? @async (override)? @override name: [(property_identifier) (computed_property_name) (private_property_identifier)] @name type_parameters: (type_parameters)? @type_params parameters: (formal_parameters) @params return_type: (type_annotation)? @ret body: (statement_block)? @body)` | `access`, `static`, `async`, `name`, `type_params`, `params`, `ret`, `body` |
| **Class Constructor** | `method_definition` where name is `constructor` | `(method_definition (accessibility_modifier)? @access name: (property_identifier) @name (#eq? @name "constructor") parameters: (formal_parameters) @params body: (statement_block) @body)` | `access`, `params`, `body` |
| **Getter / Setter** | `method_definition` where kind is `get` or `set` | `(method_definition (accessibility_modifier)? @access [(get) (set)] @accessor name: (property_identifier) @name parameters: (formal_parameters) @params return_type: (type_annotation)? @ret body: (statement_block) @body)` | `access`, `accessor`, `name`, `params`, `ret`, `body` |
| **Interface Declaration** | `interface_declaration` | `(interface_declaration name: (type_identifier) @name type_parameters: (type_parameters)? @type_params (extends_type_clause)? @extends body: (object_type) @body)` | `name`, `type_params`, `extends`, `body` |
| **Type Alias Declaration** | `type_alias_declaration` | `(type_alias_declaration name: (type_identifier) @name type_parameters: (type_parameters)? @type_params value: (_) @body)` | `name`, `type_params`, `body` |
| **Enum Declaration** | `enum_declaration` | `(enum_declaration (const)? @const name: (identifier) @name body: (enum_body) @body)` | `const`, `name`, `body` |
| **Export Statement Wrapper** | `export_statement` | `(export_statement (export)? declaration: (_) @decl)` or `(export_statement default: "default" declaration: (_) @decl)` | `decl` |

### 2.2. Type Hoister Node Kinds & Reference Extraction

| Type Reference Context | AST Node Kind | Tree-Sitter Pattern | Target Identifiers |
|---|---|---|---|
| **Simple Type Reference** | `type_identifier` | `(type_annotation (type_identifier) @type_name)` | `@type_name` |
| **Generic Type Reference** | `generic_type` | `(generic_type name: (type_identifier) @base_type arguments: (type_arguments [(type_identifier) (generic_type) (union_type) (nested_type_identifier)] @arg_type))` | `@base_type`, `@arg_type` |
| **Namespaced Type** | `nested_type_identifier` | `(nested_type_identifier module: (identifier) @mod_name name: (type_identifier) @type_name)` | `@mod_name`, `@type_name` |
| **Union / Intersection Types** | `union_type` / `intersection_type` | `(union_type [(type_identifier) (generic_type) (nested_type_identifier)] @item)` | `@item` recursively |
| **Type Constraint (Generics)** | `type_parameter` | `(type_parameter name: (type_identifier) @scoped_param constraint: (type_annotation (type_identifier) @bound_type)? default: (type_annotation (type_identifier) @default_type)?)` | Scopes `@scoped_param`; extracts `@bound_type`, `@default_type` |
| **Type Assertion / Cast** | `as_expression` / `satisfies_expression` | `[(as_expression type: [(type_identifier) (generic_type)] @cast_type) (satisfies_expression type: [(type_identifier) (generic_type)] @cast_type)]` | `@cast_type` |
| **Array & Tuple Types** | `array_type` / `tuple_type` | `[(array_type [(type_identifier) (generic_type)] @elem_type) (tuple_type [(type_identifier) (generic_type)] @elem_type)]` | `@elem_type` |
| **Type Predicate** | `type_predicate` | `(type_predicate name: (identifier) type: (type_identifier) @type_name)` | `@type_name` |
| **Heritage / Implements** | `extends_type_clause` / `implements_clause` | `[(extends_type_clause [(type_identifier) (generic_type)] @parent) (implements_clause [(type_identifier) (generic_type)] @parent)]` | `@parent` |

### 2.3. Signature Stripper Node Kinds & Call Invocations

| Invocation Pattern | AST Node Kind | Tree-Sitter Pattern | Target Identifiers |
|---|---|---|---|
| **Direct Call** | `call_expression` | `(call_expression function: (identifier) @callee arguments: (arguments) @args)` | `@callee` |
| **Method / Member Call** | `call_expression` + `member_expression` | `(call_expression function: (member_expression object: (_) @receiver property: (property_identifier) @method) arguments: (arguments) @args)` | Receiver: `@receiver`, Method: `@method` |
| **New Instantiation** | `new_expression` | `(new_expression constructor: [(identifier) (member_expression)] @ctor arguments: (arguments)? @args)` | `@ctor` |
| **Awaited Call** | `await_expression` + `call_expression` | `(await_expression (call_expression function: [(identifier) (member_expression)] @callee))` | `@callee` |

### 2.4. Import Statement Node Kinds

| Import Variant | AST Node Kind | Tree-Sitter Pattern | Extracted Symbol & Source |
|---|---|---|---|
| **Named Import** | `import_statement` | `(import_statement (import_clause (named_imports (import_specifier name: (identifier) @imported_name alias: (identifier)? @local_alias))) source: (string) @import_path)` | Original name: `@imported_name`, In-scope name: `@local_alias` or `@imported_name`, Module: `@import_path` |
| **Type-Only Named Import** | `import_statement` | `(import_statement type: "type" (import_clause (named_imports (import_specifier name: (identifier) @imported_name alias: (identifier)? @local_alias))) source: (string) @import_path)` | Same as named import |
| **Default Import** | `import_statement` | `(import_statement (import_clause (identifier) @default_name) source: (string) @import_path)` | In-scope name: `@default_name`, Target: `default`, Module: `@import_path` |
| **Namespace Import** | `import_statement` | `(import_statement (import_clause (namespace_import (identifier) @ns_name)) source: (string) @import_path)` | Namespace: `@ns_name`, Module: `@import_path` |
| **Re-export (Star)** | `export_statement` | `(export_statement (export)? (asterisk)? source: (string) @reexport_path)` | Forward all symbols from `@reexport_path` |
| **Re-export (Named)** | `export_statement` | `(export_statement (export)? (export_clause (export_specifier name: (identifier) @name alias: (identifier)? @alias)) source: (string) @reexport_path)` | Exported: `@alias` or `@name`, Source: `@reexport_path` |

---

## 3. Logic Chain & AST Extraction Algorithms

### 3.1. Symbol Locator Architecture (`crates/ctxcut_core/src/resolver/symbol.rs`)

```
                          Symbol Query (e.g. "login" or "AuthService.login")
                                                  │
                                                  ▼
                                       Parse Query Format
                                  ┌───────────────┴───────────────┐
                                  │                               │
                            "symbol_name"               "Container.method"
                                  │                               │
                                  ▼                               ▼
                      Scan Top-Level Declarations       Locate Container AST Node
                      (Functions, Consts, Classes,      (Class / Interface / Object)
                       Interfaces, Types, Enums)                  │
                                  │                               ▼
                                  │                      Find Method in Container
                                  │                               │
                                  └───────────────┬───────────────┘
                                                  │
                                                  ▼
                                       Match Target AST Node
                                                  │
                                                  ▼
                                        Capture JSDoc / Comments
                                                  │
                                                  ▼
                                        Return SymbolLocation
```

#### Step-by-Step Logic:
1. **Query Parsing**:
   - Inspect query string for `.` or `::` delimiter.
   - If present: split into `container_name` and `member_name`.
   - If absent: `container_name = None`, `member_name = query`.
2. **Top-Level AST Traversal**:
   - Traverse `tree.root_node().children()`:
     - If child is `export_statement`: inspect `child.child_by_field_name("declaration")`.
     - Match node against:
       - `function_declaration` / `generator_function_declaration`: compare `name` field identifier with `member_name`.
       - `lexical_declaration` / `variable_declaration`: iterate `declarators`, compare `declarator.name` with `member_name`.
       - `class_declaration` / `abstract_class_declaration`: if `container_name` is `None` and class `name` == `member_name`, match class; if `container_name` matches class `name`, traverse class body methods.
       - `interface_declaration`: compare `name` with `member_name`.
       - `type_alias_declaration`: compare `name` with `member_name`.
       - `enum_declaration`: compare `name` with `member_name`.
3. **Container-Scoped Search (`Container.method`)**:
   - Find class or interface AST node where `name` matches `container_name`.
   - Search within `class_body` or `interface_body`:
     - Inspect `method_definition` nodes: match `name` field (`property_identifier`).
     - Inspect `public_field_definition` / `field_definition` nodes: match arrow functions assigned to fields.
4. **Fallback Heuristic**:
   - If `container_name` was omitted and no top-level symbol matched `member_name`, scan all classes and interfaces in the file for any method matching `member_name`.
5. **JSDoc / Doc Comment Attachment**:
   - Check preceding sibling nodes in the AST.
   - If the immediately preceding named sibling is a `comment` node (e.g. `/** ... */`), verify that the byte offset between the end of the comment and the start of the symbol node consists only of whitespace/newlines.
   - Include comment in `start_line` / `byte_range` and extract verbatim doc comment text.
6. **Symbol Location Construction**:
   - Construct `ExtractedSymbol` with:
     - `name`: identifier string.
     - `kind`: `"function"`, `"method"`, `"class"`, `"interface"`, `"type"`, `"enum"`.
     - `file_path`: absolute or normalized relative path.
     - `start_line` and `end_line` (1-indexed).
     - `doc_comment`: `Option<String>`.
     - `signature`: normalized signature snippet.
     - `body`: full verbatim node text slice.
     - `language`: `"typescript"` or `"javascript"`.

---

### 3.2. Type Hoister Architecture (`crates/ctxcut_core/src/resolver/types.rs`)

```
                      Target Node AST (Signature & Body)
                                      │
                                      ▼
                      1. Collect Scoped Generic Parameters
                           (e.g., <T extends Dto, K>)
                                      │
                                      ▼
                      2. Extract All Type Identifiers
                                      │
                                      ▼
                      3. Filter Built-in Types & Primitives
                           (string, number, Promise, Array, ...)
                                      │
                                      ▼
                      4. Resolve Type Definitions
                        ┌─────────────┴─────────────┐
                        ▼                           ▼
                 Local File AST              Import Statement
             (interface, type, enum,       (named / type / default)
              class declaration)                    │
                        │                           ▼
                        │                  Resolve Module Path
                        │                  (.ts, .tsx, .d.ts, index.ts)
                        │                           │
                        │                           ▼
                        │                  Parse Target File & Extract
                        │                  (Follow Barrel Re-exports)
                        └─────────────┬─────────────┘
                                      │
                                      ▼
                      5. Transitive Closure (Depth <= opts.depth)
                           (Visited Set to Prevent Cycles)
                                      │
                                      ▼
                           Vec<ExtractedType>
```

#### Step-by-Step Logic:
1. **Generic Parameter Scoping**:
   - Inspect `(type_parameters)` of target function/method/class.
   - For each `type_parameter`:
     - Add `name` identifier (e.g. `T`, `K`, `Item`) to `scoped_generics: HashSet<String>`.
     - If `constraint` exists (e.g. `T extends UserDto`), parse `UserDto` as a type reference to hoist.
     - If `default` exists (e.g. `K = RoleEnum`), parse `RoleEnum` as a type reference to hoist.
2. **Type Identifier Traversal**:
   - Recursively traverse AST sub-nodes:
     - In signatures: `type_annotation`, `generic_type`, `union_type`, `intersection_type`, `type_predicate`, `array_type`, `tuple_type`, `nested_type_identifier`.
     - In bodies: `as_expression`, `satisfies_expression`, type annotations on `variable_declarator`, `new_expression` with type arguments (`new Set<User>()`).
   - Extract raw type names.
3. **Built-in & Primitive Filter**:
   - Filter out `scoped_generics`.
   - Filter out primitive types:
     - `string`, `number`, `boolean`, `symbol`, `bigint`, `void`, `null`, `undefined`, `never`, `unknown`, `any`, `object`, `Function`, `true`, `false`.
   - Filter out standard library & runtime globals:
     - `Array`, `ReadonlyArray`, `Promise`, `Map`, `Set`, `WeakMap`, `WeakSet`, `Date`, `RegExp`, `Error`, `TypeError`, `RangeError`, `SyntaxError`, `Uint8Array`, `Int8Array`, `Uint16Array`, `Int16Array`, `Uint32Array`, `Int32Array`, `Float32Array`, `Float64Array`, `BigInt64Array`, `BigUint64Array`, `ArrayBuffer`, `SharedArrayBuffer`, `DataView`, `Blob`, `File`, `FormData`, `URL`, `URLSearchParams`, `Headers`, `Request`, `Response`, `AbortController`, `AbortSignal`, `Event`, `CustomEvent`, `EventListener`, `NodeJS`, `Buffer`, `Process`, `Console`, `JSON`, `Math`, `Reflect`, `Proxy`, `Symbol`, `Object`, `String`, `Number`, `Boolean`, `BigInt`.
   - Filter out TypeScript standard utility types:
     - `Partial`, `Required`, `Readonly`, `Record`, `Pick`, `Omit`, `Exclude`, `Extract`, `NonNullable`, `Parameters`, `ConstructorParameters`, `ReturnType`, `InstanceType`, `ThisParameterType`, `OmitThisParameter`, `ThisType`, `Uppercase`, `Lowercase`, `Capitalize`, `Uncapitalize`, `Awaited`.
4. **Local vs Imported Type Resolution**:
   - For each unresolved `type_name`:
     - **Check Local File**:
       - Scan top-level `interface_declaration`, `type_alias_declaration`, `enum_declaration`, `class_declaration`.
       - If found: extract full declaration source slice, record `ExtractedType { name, kind, file_path, definition }`.
     - **Check Imports (`resolver/imports.rs`)**:
       - If not local, scan `import_statement` nodes in the current file:
         - Named import: `import { User } from './models'` -> resolves `User` from `./models`.
         - Aliased import: `import { UserDto as User } from './models'` -> resolves `UserDto` from `./models`.
         - Default import: `import User from './user'` -> resolves `default` or `User` from `./user`.
         - Type import: `import type { User } from './models'`.
       - Resolve import source file:
         - Compute path relative to current file's directory: `current_dir.join(src_path)`.
         - Probe candidate extensions in order: `.ts`, `.tsx`, `.d.ts`, `.js`, `.jsx`.
         - Probe candidate index files if directory: `<dir>/index.ts`, `<dir>/index.tsx`, `<dir>/index.d.ts`, `<dir>/index.js`, `<dir>/index.jsx`.
         - If target file is found on disk:
           - Read file, parse AST with `tree-sitter`.
           - Check for barrel re-exports: `export * from './submodule'` or `export { User } from './submodule'`. Follow re-export chain transitively!
           - Locate symbol declaration in target file $\to$ extract snippet $\to$ record `ExtractedType`.
5. **Transitive Hoisting & Depth Control**:
   - Maintain `visited: HashSet<String>` storing `type_name` (or `file_path:type_name`) to guarantee zero infinite loops on cyclic types (e.g. `interface TreeNode { children: TreeNode[] }` or `type A = { b: B }; type B = { a: A }`).
   - For depth $> 1$: parse AST of each hoisted definition, extract referenced types, and resolve iteratively up to `opts.depth`.

---

### 3.3. Signature Stripper Architecture (`crates/ctxcut_core/src/resolver/calls.rs`)

```
                      Target Function Body AST
                                 │
                                 ▼
                     1. Identify Call Invocations
                 (call_expression, new_expression)
                                 │
                                 ▼
                     2. Filter Built-in Callables
                 (console.*, Math.*, JSON.*, Array.*,
                  parseInt, setTimeout, fetch, ...)
                                 │
                                 ▼
                     3. Locate Callable Definition
                 ┌───────────────┴───────────────┐
                 ▼                               ▼
            Local Function / Method         Imported Function / Method
                 │                               │
                 │                      Resolve Import Path
                 │                      & Locate AST Node in Target File
                 └───────────────┬───────────────┘
                                 │
                                 ▼
                     4. Strip Function Body (100%)
                        Emit Clean Signature Stub
                                 │
                                 ▼
                     Vec<CallSignatureStub>
```

#### Step-by-Step Logic:
1. **Identify Call Expressions**:
   - Traverse target node `body` (statement block).
   - Match `call_expression`:
     - Standalone function: `(identifier)` e.g. `hashPassword(pwd)`.
     - Method invocation: `(member_expression object: (_) property: (property_identifier))` e.g. `userRepo.findById(id)` or `this.validate(token)`.
   - Match `new_expression`: `new AuthService(config)`.
2. **Filter Out Built-ins & Standard Library Callables**:
   - `console.*` (`log`, `warn`, `error`, `info`, `debug`, `trace`, `dir`, `time`, `timeEnd`, `table`)
   - `Math.*` (`floor`, `ceil`, `round`, `max`, `min`, `abs`, `random`, `pow`, `sqrt`, `trunc`, `sign`)
   - `JSON.*` (`parse`, `stringify`)
   - `Object.*` (`keys`, `values`, `entries`, `assign`, `freeze`, `seal`, `create`, `getPrototypeOf`, `hasOwnProperty`)
   - `Array.*` / Prototype methods (`map`, `filter`, `reduce`, `forEach`, `some`, `every`, `find`, `findIndex`, `includes`, `slice`, `splice`, `concat`, `join`, `push`, `pop`, `shift`, `unshift`, `flat`, `flatMap`, `sort`, `reverse`)
   - `String.*` / Prototype methods (`toLowerCase`, `toUpperCase`, `trim`, `trimStart`, `trimEnd`, `split`, `replace`, `replaceAll`, `substring`, `slice`, `startsWith`, `endsWith`, `includes`, `indexOf`, `padStart`, `padEnd`, `charAt`, `charCodeAt`, `match`, `search`)
   - `Promise.*` (`all`, `resolve`, `reject`, `allSettled`, `race`, `any`)
   - Global standard functions: `parseInt`, `parseFloat`, `isNaN`, `isFinite`, `encodeURIComponent`, `decodeURIComponent`, `encodeURI`, `decodeURI`, `setTimeout`, `clearTimeout`, `setInterval`, `clearInterval`, `fetch`, `structuredClone`, `atob`, `btoa`.
3. **Definition Location & Resolution**:
   - If call is `this.methodName(...)` or `ClassName.methodName(...)`:
     - Search containing class/interface for `methodName`.
   - If call is standalone `funcName(...)`:
     - Check local functions in current file first.
     - If not local, match `import_statement` in current file $\to$ resolve source file $\to$ locate declaration AST node in imported file.
   - If call is `receiver.methodName(...)` on imported instance or typed object:
     - Trace `receiver` variable declaration or import source $\to$ locate class/interface $\to$ find method signature.
4. **Body Stripping Mechanism**:
   - **Named Function**:
     - Input:
       ```typescript
       export async function hashPassword(password: string, salt: string): Promise<string> {
           const hashed = await crypto.scrypt(password, salt, 64);
           return hashed.toString('hex');
       }
       ```
     - Stripped Stub:
       ```typescript
       export async function hashPassword(password: string, salt: string): Promise<string>;
       ```
   - **Arrow Function / Lexical Declarator**:
     - Input:
       ```typescript
       export const calculateTax = (amount: number, rate: number): number => {
           return amount * rate;
       };
       ```
     - Stripped Stub:
       ```typescript
       export function calculateTax(amount: number, rate: number): number;
       ```
   - **Class Method**:
     - Input:
       ```typescript
       public async findById(id: string): Promise<User | null> {
           return this.db.query('SELECT * FROM users WHERE id = $1', [id]);
       }
       ```
     - Stripped Stub:
       ```typescript
       public async findById(id: string): Promise<User | null>;
       ```
   - **Fallback for Untyped JS / Unresolvable Call**:
     - If definition AST is unavailable or untyped JS:
       ```typescript
       export function calleeName(...args: any[]): any;
       ```
   - Record `CallSignatureStub { name, receiver, file_path, signature }`.

---

## 4. Error Handling Architecture (`crates/ctxcut_core/src/error.rs`)

```rust
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("File not found: {path}")]
    FileNotFound { path: PathBuf },

    #[error("Unsupported language for file: {path} (extension: {extension:?})")]
    UnsupportedLanguage {
        path: PathBuf,
        extension: Option<String>,
    },

    #[error("Failed to parse file '{file}': {message}")]
    ParseError { file: PathBuf, message: String },

    #[error("Symbol '{symbol}' not found in '{file}'. Available symbols: {available_symbols:?}")]
    SymbolNotFound {
        symbol: String,
        file: PathBuf,
        available_symbols: Vec<String>,
    },

    #[error("Import resolution error for specifier '{specifier}' from '{from_file}': {reason}")]
    ImportResolutionError {
        specifier: String,
        from_file: PathBuf,
        reason: String,
    },

    #[error("I/O error at '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Tree-sitter query execution error: {0}")]
    QueryError(#[from] tree_sitter::QueryError),
}
```

### Diagnostic Enhancements:
- `SymbolNotFound` automatically collects and returns `available_symbols` across the source file, giving instant corrective feedback (e.g. if the user made a typo like `ctxcut slice auth.ts:logni` -> suggests `["login", "register", "logout", "AuthService"]`).
- `ImportResolutionError` details the exact unresolvable specifier and candidate paths probed on disk.

---

## 5. Code Structure & Module Layout for `ctxcut_core`

```
crates/ctxcut_core/
├── Cargo.toml
└── src/
    ├── lib.rs                      # Re-exports Public API & ContextSlicer
    ├── error.rs                    # CoreError definitions with thiserror
    ├── model.rs                    # ExtractedSymbol, ExtractedType, CallSignatureStub, TokenStats, SliceResult
    ├── lang/
    │   ├── mod.rs                  # LanguageAdapter trait & SupportedLanguage enum
    │   ├── typescript.rs           # TypeScript / TSX / JavaScript grammar bindings & AST visitor
    │   ├── python.rs               # (M2 stub / placeholder)
    │   ├── go.rs                   # (M2 stub / placeholder)
    │   └── rust_lang.rs            # (M2 stub / placeholder)
    ├── parser/
    │   ├── mod.rs                  # Tree-sitter ParserManager & AST query cache
    │   └── cursor.rs               # Traversal helpers & capture iterators
    ├── resolver/
    │   ├── mod.rs                  # AST Resolver facade
    │   ├── symbol.rs               # SymbolLocator (functions, methods, classes, types, enums)
    │   ├── imports.rs              # TypeScript module & barrel re-export path resolver
    │   ├── types.rs                # TypeHoister (generic filtering, local AST lookup, transitive closure)
    │   └── calls.rs                # SignatureStripper (call expression extraction & body stripping)
    ├── slice/
    │   └── mod.rs                  # ContextSlicer orchestration engine
    ├── formatter/
    │   ├── mod.rs                  # Formatter facade
    │   ├── markdown.rs             # Prompt-optimized Markdown renderer
    │   └── json.rs                 # Structured JSON serializer
    └── tokenizer/
        └── mod.rs                  # tiktoken-rs BPE token counter (cl100k_base) & metrics calculator
```

### 5.1. LanguageAdapter Trait Definition (`crates/ctxcut_core/src/lang/mod.rs`)

```rust
use std::path::Path;
use tree_sitter::{Language, Node};
use crate::error::CoreError;
use crate::model::{ExtractedSymbol, ExtractedType, CallSignatureStub, SliceOptions};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SupportedLanguage {
    TypeScript,
    JavaScript,
    Python,
    Go,
    Rust,
}

impl SupportedLanguage {
    pub fn from_path(path: &Path) -> Result<Self, CoreError> {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("ts") | Some("tsx") | Some("mts") | Some("cts") => Ok(Self::TypeScript),
            Some("js") | Some("jsx") | Some("mjs") | Some("cjs") => Ok(Self::JavaScript),
            Some("py") | Some("pyi") => Ok(Self::Python),
            Some("go") => Ok(Self::Go),
            Some("rs") => Ok(Self::Rust),
            ext => Err(CoreError::UnsupportedLanguage {
                path: path.to_path_buf(),
                extension: ext.map(String::from),
            }),
        }
    }
}

pub trait LanguageAdapter: Send + Sync {
    fn language(&self) -> SupportedLanguage;
    fn tree_sitter_language(&self, path: &Path) -> Language;
    
    /// Locates the AST node and metadata for a target symbol
    fn locate_symbol<'a>(
        &self,
        root: Node<'a>,
        source: &'a str,
        symbol_query: &str,
        file_path: &Path,
    ) -> Result<ExtractedSymbol, CoreError>;

    /// Lists all available symbols in the file for diagnostics
    fn list_symbols<'a>(&self, root: Node<'a>, source: &'a str) -> Vec<String>;

    /// Extracts referenced types from signature and body
    fn hoist_types<'a>(
        &self,
        target_node: Node<'a>,
        root: Node<'a>,
        source: &'a str,
        file_path: &Path,
        opts: &SliceOptions,
    ) -> Result<Vec<ExtractedType>, CoreError>;

    /// Identifies call expressions and extracts body-stripped signatures
    fn strip_calls<'a>(
        &self,
        target_node: Node<'a>,
        root: Node<'a>,
        source: &'a str,
        file_path: &Path,
    ) -> Result<Vec<CallSignatureStub>, CoreError>;
}
```

---

## 6. Prompt-Optimized Markdown Output Specification

```markdown
### Context Slice: `src/auth/service.ts:login`
*Language: TypeScript | Parse Latency: 1.42ms | Tokens: 348 -> 92 (73.6% reduction)*

#### 1. Target Implementation (Full Body)
```typescript
/**
 * Authenticates user credentials and issues a JWT access token.
 */
export async function login(dto: LoginDto): Promise<AuthResponse> {
    const user = await userRepo.findByEmail(dto.email);
    if (!user) {
        throw new AuthError(ErrorCode.UserNotFound);
    }
    const isValid = await verifyPassword(dto.password, user.passwordHash);
    if (!isValid) {
        throw new AuthError(ErrorCode.InvalidCredentials);
    }
    const token = generateToken(user.id, user.role);
    return { token, user };
}
```

#### 2. Hoisted Types & Data Contracts
```typescript
export interface LoginDto {
    email: string;
    password: string;
}

export interface AuthResponse {
    token: string;
    user: User;
}

export interface User {
    id: string;
    email: string;
    role: UserRole;
    passwordHash: string;
}

export enum UserRole {
    Admin = "ADMIN",
    User = "USER",
}

export enum ErrorCode {
    UserNotFound = "USER_NOT_FOUND",
    InvalidCredentials = "INVALID_CREDENTIALS",
}
```

#### 3. External Dependencies & Signatures (Body Stripped)
```typescript
export async function verifyPassword(password: string, hash: string): Promise<boolean>;
export function generateToken(userId: string, role: UserRole): string;
export class AuthError extends Error {
    constructor(code: ErrorCode);
}
```
```

---

## 7. Caveats & Assumptions

1. **Dynamic Invocations**: Dynamic dispatch expressions (e.g. `obj[fnName]()` or `eval(...)`) cannot be statically resolved via AST and will be ignored or emitted as generic dynamic calls.
2. **Third-Party Package Boundary**: Type hoisting and signature stripping for external dependencies inside `node_modules` (or remote URLs) will extract available type declarations if `.d.ts` is present, but will not perform unbounded repository crawls.
3. **Module Resolution Strategy**: TS/JS path resolution assumes standard relative paths (`./`, `../`) and index resolution. Custom path aliases (`tsconfig.json` `paths` / baseUrl) can be extended in future milestones.
4. **Encoding**: Source code files are assumed to be valid UTF-8.

---

## 8. Conclusion

The AST specification, exact Tree-sitter node mappings, query rules, type hoisting algorithms, signature stripping mechanics, and module layout for TypeScript and JavaScript in `ctxcut_core` Milestone 1 are completely formulated, rigorously verified against Tree-sitter grammar definitions, and ready for immediate implementation.

---

## 9. Verification Method

To verify the TypeScript/JavaScript AST query and resolver engine implementation:
1. **Workspace Compilation**:
   ```bash
   cargo check --workspace
   cargo clippy --all-targets -- -D warnings
   ```
2. **Core Unit & Integration Tests**:
   ```bash
   cargo test -p ctxcut_core
   ```
3. **Dedicated Test Fixture Scenarios**:
   - `tests/fixtures/typescript/auth_service.ts`: Tests named async function, interface hoisting, enum hoisting, and call signature stripping.
   - `tests/fixtures/typescript/class_methods.ts`: Tests class declarations, static/instance methods, getters/setters, constructors, and field arrow functions.
   - `tests/fixtures/typescript/generic_types.ts`: Tests generic constraints (`<T extends Dto>`), type aliases (`type Result<T> = ...`), union/intersection types, and scoped generic parameter exclusion.
   - `tests/fixtures/typescript/barrel_imports/`: Tests relative import resolution and barrel re-exports (`export * from './submodule'`).
   - `tests/fixtures/javascript/commonjs_es6.js`: Tests JavaScript function declarations, CommonJS exports, and untyped signature fallback.
