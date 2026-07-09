# Phase 24 交接文件

> 狀態：待實作  
> 前置：Phase 23 完成（280 tests passing）  
> 目標：`oo` CLI 新增 `eval` 和 `inspect` 子命令

---

## 概覽

**位置**：`crates/oo/src/main.rs`（唯一修改檔案）

| 子命令 | 用途 |
|:-------|:-----|
| `oo eval <expr>` | 行內求值一個 nlang 運算式，印出結果 |
| `oo inspect <caid>` | 從本地 store 查詢任意 CAID 的值 |

**注意**：CLI binary 不加 Rust 測試檔。驗證方式：
```bash
cd /mnt/d/Workspace/ai_ai/nlang/nlang-tools
cargo build --manifest-path crates/oo/Cargo.toml
# 確認 build 成功，再手動測試
```

---

## Task 1：`oo eval <expr>`

### 語義

```bash
oo eval "1 + 2"          # => 3
oo eval "\"hello\" |> str.to_upper"   # => "HELLO"
oo eval "{x: 10, y: 20}"             # => {x: 10, y: 20}
```

- 在全新 universe（含系統 builtins）中求值
- 將輸入包裝為 `__eval_result = <expr>` 再執行
- 印出 `to_nlang(0)` 格式的結果
- 若不在 `.oo` 專案目錄，仍可執行（fallback 到 `new_in_memory`）

### clap 結構變更

在 `Commands` enum 加入：
```rust
Eval {
    /// nlang expression to evaluate
    expr: String,
},
```

在 `main()` 的 match 加入：
```rust
Commands::Eval { expr } => run_eval(expr),
```

### 實作

```rust
fn run_eval(expr: String) -> anyhow::Result<()> {
    let cur = std::env::current_dir()?;
    // Graceful fallback: work outside .oo projects too
    let engine = Ouroboros::init(&cur)
        .unwrap_or_else(|_| Ouroboros::new_in_memory());

    let mut universe = Universe::new(None, engine.root_with_system());

    // Wrap bare expression as a field definition
    let wrapped = format!("__eval_result = {}", expr.trim());
    let program = parse_program(&wrapped)
        .map_err(|e| anyhow::anyhow!("Parse error: {}", e))?;

    for f in &program.fields {
        if let Err(e) = universe.evolve(&engine, f) {
            anyhow::bail!("Eval error: {:?}", e);
        }
    }

    let path = parse_path_only("__eval_result")?;
    let result = universe.observe(&engine, &path);
    println!("{}", result.to_nlang(0));
    Ok(())
}
```

### 注意事項

- `Ouroboros::new_in_memory()` 需確認是公開方法（應與 Rust 測試中使用的相同）。若不存在，用 `Ouroboros::init(&cur)?`（僅在專案目錄有效）。
- `parse_path_only` 已在 `main.rs` 底部定義，直接複用。
- `__eval_result` 是內部暫名，不會與使用者的欄位衝突（有 `__` 前綴）。
- 若使用者輸入 `oo eval "x = 1"` — 包裝後是 `__eval_result = x = 1`，可能導致 parse 問題。這是已知限制，文件說明 `eval` 只接受運算式。

---

## Task 2：`oo inspect <caid>`

### 語義

```bash
oo inspect hash:sha256:v2:masa:sketch:deadbeef...
# 輸出：
# CAID:  hash:sha256:v2:masa:sketch:deadbeef...
# MASA:  masa:fk:abcd1234
# Sketch: <base64>
# 
# {
#   x: 10
#   y: 20
# }
```

- 解析 CAID 字串
- 從本地 store 查詢
- 印出結構化資訊 + 值的 `to_nlang(0)`
- CAID 不存在時，清楚報錯

### clap 結構變更

```rust
Inspect {
    /// CAID to look up (format: hash:sha256:v2:...)
    caid: String,
},
```

在 `main()` 加入：
```rust
Commands::Inspect { caid } => run_inspect(caid),
```

### 實作

```rust
fn run_inspect(caid_str: String) -> anyhow::Result<()> {
    let cur = std::env::current_dir()?;
    let engine = Ouroboros::init(&cur)
        .unwrap_or_else(|_| Ouroboros::new_in_memory());

    let hash = ContentHash::parse(&caid_str)
        .map_err(|_| anyhow::anyhow!("Invalid CAID format: {}", caid_str))?;

    let val = engine.store.get_value(&hash)
        .map_err(|_| anyhow::anyhow!("CAID not found in local store: {}", caid_str))?;

    // Header
    println!("CAID:   {}", caid_str);
    println!("MASA:   {}", hash.masa_ref);
    if !hash.lattice_sketch.is_empty() {
        // Show first 32 chars of sketch for readability
        let sketch_preview = if hash.lattice_sketch.len() > 32 {
            format!("{}...", &hash.lattice_sketch[..32])
        } else {
            hash.lattice_sketch.clone()
        };
        println!("Sketch: {}", sketch_preview);
    }
    println!();

    // Value
    println!("{}", val.to_nlang(0));
    Ok(())
}
```

### 注意事項

- `ContentHash` 的欄位：`masa_ref: MasaRef`（Display 已實作）、`lattice_sketch: String`（Base64）、`digest: String`。
- `engine.store.get_value(&hash)` 返回 `Result<Value, _>`，錯誤表示 CAID 不在本地 store（可能在 peer）。
- `hash.lattice_sketch` 可能很長，preview 截斷 32 字元。

---

## 完整 Commands enum（修改後）

```rust
#[derive(Subcommand)]
enum Commands {
    Run { #[arg(required = true)] files: Vec<PathBuf>, #[arg(short, long)] observe: Option<String>, #[arg(short, long)] format: bool },
    Evolve { #[arg(required = true)] files: Vec<PathBuf> },
    Test { #[arg(long)] static_only: bool, #[arg(short, long)] pattern: Option<String>, files: Vec<PathBuf> },
    Repl, Status, Log,
    Commit { #[arg(short, long)] message: Option<String> },
    Refine {
        #[arg(short, long, required = true, num_args = 1..)]
        source: Vec<String>,
        #[arg(short, long, required = true, num_args = 1..)]
        target: Vec<String>,
        #[arg(long)]
        sign: bool,
        #[arg(short, long)]
        message: Option<String>,
    },
    Fmt { file: PathBuf, #[arg(short, long)] write: bool },
    Serve { #[arg(short, long, default_value_t = 8080)] port: u16 },
    /// Evaluate a nlang expression inline
    Eval {
        /// nlang expression to evaluate (wrap in quotes for shell safety)
        expr: String,
    },
    /// Inspect a value in the local store by CAID
    Inspect {
        /// CAID string (hash:sha256:v2:...)
        caid: String,
    },
}
```

## 完整 main() match（修改後）

```rust
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run { files, observe, format } => run_one_shot(files, observe, format),
        Commands::Evolve { files } => run_evolve(files),
        Commands::Fmt { file, write } => run_fmt(file, write),
        Commands::Serve { port } => run_serve(port),
        Commands::Status => run_status(),
        Commands::Log => run_log(),
        Commands::Commit { message } => run_commit(message),
        Commands::Refine { source, target, sign, message } => run_refine(source, target, sign, message),
        Commands::Repl => run_repl(),
        Commands::Test { static_only, pattern, files } => run_test(static_only, pattern, files),
        Commands::Eval { expr } => run_eval(expr),
        Commands::Inspect { caid } => run_inspect(caid),
    }
}
```

---

## 驗證步驟

### 1. 編譯確認
```bash
cd /mnt/d/Workspace/ai_ai/nlang/nlang-tools
cargo build --manifest-path crates/oo/Cargo.toml 2>&1
# 必須 0 errors，0 warnings（或只有 unused 警告）
```

### 2. Help 確認
```bash
./target/debug/oo --help
# 應顯示 eval 和 inspect 子命令
./target/debug/oo eval --help
./target/debug/oo inspect --help
```

### 3. eval 功能測試
```bash
# 基本運算
./target/debug/oo eval "1 + 2"
# 預期輸出：3

# 字串操作
./target/debug/oo eval "\"hello\""
# 預期輸出："hello"

# Combo
./target/debug/oo eval "{x: 10, y: 20}"
# 預期輸出：類似 {x: 10, y: 20}

# 無效運算式
./target/debug/oo eval "??invalid??"
# 預期：Parse error 訊息
```

### 4. inspect 功能測試
```bash
# 先取得一個 CAID（用 repl 建立一個值）
# 或用 genesis 的已知 CAID 測試
./target/debug/oo inspect hash:sha256:v2:top::aaaa
# 預期：CAID not found（CAID 格式正確但不存在）

./target/debug/oo inspect "not_a_caid"
# 預期：Invalid CAID format 錯誤訊息
```

### 5. Interpreter test suite 不受影響
```bash
cargo test --manifest-path crates/interpreter/Cargo.toml
# 預期：280 tests, 0 failed（CLI 改動不影響 interpreter）
```

---

## 使用場景說明

**`oo eval`** 的典型用途：
```bash
# 快速測試 builtin
oo eval "str.format(\"Hello, {}!\", [\"Alice\"])"

# 驗算數學
oo eval "math.sqrt(144.0)"

# 探索 Combo 結構
oo eval "{name: \"nlang\", version: 1}"
```

**`oo inspect`** 的典型用途：
- 在 `oo run --observe <path>` 輸出 CAID 後，進一步查看該 CAID 的內容
- 追蹤 #refine 後 shadow_affected 列表中的歷史 commit 值
- 驗證 `disc.advertise` 後 GBB 的 CAID 是否可被 inspect 到
