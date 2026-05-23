# nlang 引擎架构与功能清单

> 本文档为 nlang-tools 引擎的技术架构概览，供新贡献者快速定位切入点。

---

## 1. 项目结构

```
nlang-tools/
├── crates/
│   ├── parser/          # AST 与语法解析
│   │   ├── src/lib.rs   # Parser 入口 (Pest)
│   │   ├── src/ast.rs   # AST 类型定义
│   │   └── src/n.pest   # Pest 语法定义
│   │
│   ├── interpreter/     # 核心运行时引擎
│   │   ├── src/lib.rs           # Ouroboros 引擎、EvalContext
│   │   ├── src/value.rs         # Value、ComboVal、EffectTag、ContentHash
│   │   ├── src/eval.rs          # 表达式求值
│   │   ├── src/unify.rs         # 统一化 (Meet) 运算
│   │   ├── src/dispatch.rs      # 态射模式匹配
│   │   ├── src/complement.rs    # 正交补运算 (!)
│   │   ├── src/type_constraint.rs # 类型约束检查
│   │   ├── src/universe.rs      # Universe 状态管理
│   │   ├── src/storage.rs       # CAID 对象存储
│   │   ├── src/observation.rs   # 资源耗尽处理
│   │   └── src/builtins/        # 内置模块
│   │       ├── mod.rs           # 注册入口
│   │       ├── math.rs          # ~%Math, ~%Complex
│   │       ├── list.rs          # ~%List
│   │       ├── string.rs        # ~%Str
│   │       ├── cond.rs          # ~%Cond
│   │       ├── time.rs          # ~%Time
│   │       ├── reflection.rs    # ~%Refl
│   │       ├── engine.rs        # ~%Engine
│   │       └── disc.rs          # ~%Disc (发现协议)
│   │
│   └── oo/              # CLI 工具入口
│       ├── src/main.rs          # CLI 命令处理
│       └── src/static_analyzer.rs # 静态分析
│
└── docs/                # 文档目录
    ├── implementation-status.md  # 实现状态分析
    ├── engine-architecture.md    # 本文档
    └── feature-roadmap.md        # 功能路线图
```

---

## 2. Crate 概览

### 2.1 `nlang-parser`

**用途**：将源文本转换为 AST

**关键公开 API**：
```rust
pub fn parse_program(input: &str) -> Result<Program, Box<dyn Error>>
pub fn parse_expr_only(input: &str) -> Result<Expr, Box<dyn Error>>
pub fn parse_field(pair: Pair<Rule>) -> Result<Field, Box<dyn Error>>
```

**依赖**：`pest`, `pest_derive`, `serde`, `num-bigint`

**切入点**：
- 新增语法规则 → `n.pest`
- 新增 AST 类型 → `ast.rs` ExprKind enum
- 新增解析逻辑 → `lib.rs` parse_expr

---

### 2.2 `nlang-interpreter`

**用途**：核心运行时，实现求值、统一化、存储、内置操作

**关键类型**：

| 类型 | 文件 | 说明 |
|:-----|:-----|:-----|
| `Ouroboros` | `lib.rs` | 主引擎，包含 store、registry、peers、identity |
| `EvalContext` | `lib.rs` | 求值上下文：root、scopes、fuel、depth 等 |
| `Universe` | `universe.rs` | 状态管理：head、root、staged、dirty |
| `Value` | `value.rs` | 核心值类型 enum |
| `ComboVal` | `value.rs` | Combo 结构体 |
| `EffectTag` | `value.rs` | 效果标签 (Pure/State/IO/NonDet) |
| `ContentHash` | `value.rs` | CAID (SHA256) |
| `BuiltinFn` | `lib.rs` | 内置函数类型签名 |

**切入点**：
- 新增 Value 类型 → `value.rs` Value enum + AtomKind
- 新增统一化规则 → `unify.rs` unify_internal
- 新增求值逻辑 → `eval.rs` eval_internal
- 新增内置模块 → `builtins/` 新建模块 + `mod.rs` 注册

---

### 2.3 `oo` (CLI)

**用途**：命令行工具，REPL，测试运行

**CLI 命令**：
| 命令 | 功能 |
|:-----|:-----|
| `oo run <files>` | 单次执行 |
| `oo evolve <files>` | 演化暂存区 |
| `oo test <files>` | 运行测试 |
| `oo repl` | 交互 REPL |
| `oo status` | 显示暂存状态 |
| `oo log` | 显示 Commit 历史 |
| `oo commit` | 提交暂存区 |
| `oo fmt <file>` | 格式化源码 |
| `oo serve` | NDP 网络服务 |

---

## 3. 核心数据结构

### 3.1 Value 类型层次

```
Value (enum)
├── Top                        # 万有子空间 _
├── Atom(AtomKind, EffectTag, Option<i64>)
│   ├── Int(BigInt)            # 任意精度整数
│   ├── Float(f64)             # IEEE 754 双精度
│   ├── Complex(f64, f64)      # 复数 (re + im*i)
│   ├── Str(String)            # 字符串
│   ├── Tag(String)            # 标签 #true, #false
│   ├── TagStart               # 序位起点 #_|_
│   ├── TagEnd                 # 序位终点 #_
│   ├── Regex(String)          # 正则表达式
│   ├── PathLit(String)        # 路径字面量
│   ├── Bytes(Vec<u8>)         # 位元组
│   ├── Uri(String)            # URI
│   └── Time(String)           # 时间
│
├── Combo(ComboVal)            # 组合结构 {}
├── Union(Vec<Value>)          # 联集态 A | B
├── Thunk { expr, closure, effect } # 惰性求值
├── Code(Box<Expr>)            # 未执行代码
└── Bottom(Box<BottomDetail>)  # 矛盾 _|_ (含 %cause)
```

### 3.2 ComboVal 结构

```rust
pub struct ComboVal {
    pub data: IndexMap<String, Value>,      // 普通字段
    pub types: IndexMap<String, Value>,     // 类型约束 @prefix
    pub rules: IndexMap<String, Value>,     // 规则/态射 /prefix
    pub meta: IndexMap<String, Value>,      // 元数据 %prefix
    pub system: IndexMap<String, Value>,    // 系统 ~%prefix
    pub local: IndexMap<String, Value>,     // 私有 ~prefix
    pub closed: bool,                       // Cocoon 模式 {{}}
    pub effect: EffectTag,                  // 效果传播
    pub relations: Vec<ValRelation>,        // 序位关系 <, >, <=, >=
}
```

### 3.3 前缀系统

| 前缀 | 符号 | 命名空间 | 示例 |
|:-----|:-----|:---------|:-----|
| System | `~%` | 系统内置 | `~%Math./add` |
| Private | `~` | 私有字段 | `~temp` |
| Logic | `/` | 规则/态射 | `/handler` |
| Type | `@` | 类型约束 | `@int` |
| Meta | `%` | 元数据 | `%morphism` |
| Data | 无 | 普通数据 | `name` |

---

## 4. 核心算法

### 4.1 统一化 (Meet `&`)

**位置**：`unify.rs:unify_internal`

**流程**：
```
unify(A, B):
  1. A == B → 返回 A
  2. A == Top → 返回 B
  3. A == Bottom 或 B == Bottom → 返回 Bottom
  4. A, B 都是 Atom → 检查相等性
  5. A 是 Atom, B 是 Combo → 原子同构展开 { %val: A }
  6. A, B 都是 Combo → 递归字段合并
     - 共有字段: unify(A.p, B.p)
     - A 独有字段 + B 开放 → 保留
     - A 有字段 + B Cocoon → Bottom
  7. A 或 B 是 Union → 极小元素筛选
```

**关键概念**：
- **Cocoon 封闭**：未定义字段被视为 Bottom，拒绝外部扩张
- **极小元素规则**：保留最特定的匹配项
- **Memoization**：避免重复计算

---

### 4.2 求值流程

**位置**：`eval.rs:eval_internal`

**主要分支**：
| ExprKind | 处理 |
|:---------|:-----|
| `Atom` | 直接返回 Value |
| `Path` | 在 root/scopes 中观测路径 |
| `Apply` | 调用态射 |
| `Pipe` | `a |> b` = `b(a)` |
| `Morphism` | 创建闭包 |
| `Combo` | 构建 ComboVal |
| `Meet` | `a & b` → unify |
| `Join` | `a | b` → Union |
| `Diff` | `a \ b` → a & !b |
| `Complement` | `!a` → complement |
| `Add/Sub/Mul/Div/Rem` | 数学运算 |
| `Eq/Ne/Lt/Gt/Lte/Gte` | 比较 → #true/#false |
| `List` | 构建 List Combo |
| `Lens` | 字段访问 |
| `Ternary` | `cond ? then : else` |

---

### 4.3 态射派发

**位置**：`dispatch.rs:dispatch_morphism`

**流程**：
```
dispatch_morphism(morphism, arg):
  1. 从 %rules 提取所有分支
  2. 对每个分支进行模式匹配 (unify pattern with arg)
  3. 筛选成功匹配的分支
  4. 应用极小元素规则 (移除被包含的分支)
  5. 若单一极小元素 → 执行 body
  6. 若多个极小元素 → 返回 Union
```

**模式类型**：
| Pattern | 匹配规则 |
|:--------|:---------|
| `_` (通配符) | 匹配任意 |
| `it` | 回退匹配 |
| `@type` | 类型约束检查 |
| 字面值 | 精确匹配 |

---

### 4.4 CAID 计算

**位置**：`value.rs:content_hash()`

**当前实现**：
```rust
fn content_hash(&self) -> ContentHash {
    // SHA256 hash of canonical JSON serialization
    // + horizon_salt for effect-bearing values
}
```

**需扩展** (REAL_03)：
- BN/ 位元流序列化
- Lattice Sketch 谱特征
- 格式：`hash:<algo>:v<ver>:<sketch>:<digest>`

---

## 5. 内置模块详解

### 5.1 添加新内置模块的步骤

1. **创建模块文件**：`builtins/my_module.rs`

```rust
use std::sync::Arc;
use crate::{Ouroboros, EvalContext, BuiltinFn, Value};

pub fn register_my_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    m.insert("my.func".to_string(), Arc::new(|arg, oo, ctx| {
        // 实现逻辑
        Value::Top
    }) as Arc<BuiltinFn>);
}
```

2. **注册到 mod.rs**：
```rust
mod my_module;
// 在 create_default_builtins() 中：
my_module::register_my_builtins(&mut m);
```

3. **暴露到 root** (`lib.rs:root_with_system()`)：
```rust
// 创建态射 Combo 并添加到 ~%My
```

### 5.2 现有模块清单

| 模块 | 文件 | 态射 | 行数 |
|:-----|:-----|:-----|:-----|
| Math | `math.rs` | `/add`, `/sub`, `/mul`, `/div`, `/rem`, `/abs`, `/pow`, `/bits`, `/random`, 位运算 | 212 |
| Complex | `math.rs` | `/conj`, `/phase`, `/real`, `/imag` | - |
| List | `list.rs` | `/len`, `/at`, `/concat`, `/reverse`, `/slice`, `/zip`, `/sort`, `/map`, `/fold`, `/filter` | 198 |
| Str | `string.rs` | `/concat`, `/len`, `/trim`, `/split`, `/join`, `/replace`, `/to_lower`, `/to_upper`, `/starts_with`, `/ends_with`, `/contains` | 117 |
| Cond | `cond.rs` | `/if`, `/cond`, `/match` | 48 |
| Disc | `disc.rs` | `/connect`, `/fetch`, `/identify` | 82 |
| Refl | `reflection.rs` | `/keys`, `/has`, `/is_cocoon`, `/type_of` | 68 |
| Engine | `engine.rs` | `/observe`, `/save` | 23 |
| Time | `time.rs` | `/now` | 10 |

---

## 6. 效果系统

### 6.1 EffectTag 层级

| 标签 | 值 | 说明 |
|:-----|:---|:-----|
| Pure | 0 | 确定性、无副作用、可缓存 |
| State | 1 | 读写程序状态 |
| IO | 2 | 外部 I/O 操作 |
| NonDet | 3 | 非确定性 (random, time) |

### 6.2 效果传播规则

- 效果通过统一化传播：`max(effect_A, effect_B)`
- Thunk 携带预测效果
- 效果影响 CAID 计算（通过 horizon_salt）

---

## 7. 测试系统

### 7.1 测试格式

```nlang
test_basic: 1 + 1 == 2
test_morph: (x -> x + 1) 5 == 6
test_error: 1 & 2 == _|_
```

### 7.2 运行测试

```bash
oo test file.n           # 全部测试
oo test --static-only    # 仅静态分析
```

### 7.3 静态分析 (`static_analyzer.rs`)

检测违规：
- 随机性依赖 (`#nondet`)
- 环境依赖 (`#io`, `#state`)
- 类型冲突

---

## 8. 扩展点索引

### 想要... → 查看文件

| 目标 | 文件 | 关键位置 |
|:-----|:-----|:---------|
| 新增语法 | `parser/n.pest`, `parser/ast.rs` | Rule enum, ExprKind enum |
| 新增值类型 | `interpreter/value.rs` | Value enum, AtomKind enum |
| 新增统一化规则 | `interpreter/unify.rs` | unify_internal |
| 新增求值逻辑 | `interpreter/eval.rs` | eval_internal |
| 新增正交补规则 | `interpreter/complement.rs` | complement |
| 新增内置函数 | `interpreter/builtins/*.rs` | 新建模块 |
| 新增 CLI 命令 | `oo/main.rs` | Commands enum |
| 新增静态检查 | `oo/static_analyzer.rs` | StaticViolation enum |

---

## 9. 与规格书的对应

| 规格章节 | 引擎对应文件 |
|:---------|:-------------|
| SPEC_01 (格论基础) | `value.rs`, `unify.rs`, `complement.rs` |
| SPEC_03 (Combo 系统) | `value.rs:ComboVal` |
| SPEC_06 (统一化逻辑) | `unify.rs`, `dispatch.rs` |
| SPEC_07 (逻辑与管道) | `eval.rs:Pipe`, `dispatch.rs` |
| SPEC_09 (标准库) | `builtins/*.rs` |
| SPEC_10 (演化与 Commit) | `universe.rs`, `storage.rs` |
| SPEC_11 (反射与合成) | `builtins/reflection.rs` |
| SPEC_13 (OODP) | `builtins/disc.rs` |
| REAL_03 (CAID) | `value.rs:content_hash()` |

---

## 10. 调试与开发建议

### 10.1 常用调试手段

```bash
# 运行单个文件并观察输出
oo run test.n

# 进入 REPL 交互调试
oo repl

# 查看暂存区状态
oo status

# 查看 Commit 历史
oo log
```

### 10.2 Rust 层面调试

在 `eval.rs` 或 `unify.rs` 中添加：
```rust
println!("DEBUG: {:?}", value);
```

### 10.3 性能瓶颈

- `unify.rs` 的递归合并 → 需要 memoization
- `dispatch.rs` 的模式匹配 → 需要索引优化
- `value.rs` 的 CAID 计算 → 需要缓存

---

## 11. 关键概念速查

| 概念 | 符号 | 说明 |
|:-----|:-----|:-----|
| Meet (合并) | `&` | 子空间交集，收敛 |
| Join (联集) | `|` | 子空间併元，叠加 |
| Orthocomplement | `!` | 正交补，否定 |
| Cocoon | `{{}}` | 封闭 Combo，拒绝外部扩张 |
| CAID | ContentHash | 内容定址标识符 |
| Bohrification | - | 视角切换，局部分配律 |
| Trinity Isomorphism | - | Atom ↔ Combo via %val |
| EML | `/eml` | 数学 LUCA (exp-ln) |
| LADD | - | 谱几何分布式发现 |

---

> **快速入门建议**：
> 1. 先读 `value.rs` 理解核心类型
> 2. 再读 `unify.rs` 理解统一化
> 3. 最后读 `eval.rs` 理解求值流程
> 4. 用 `oo repl` 实验验证理解