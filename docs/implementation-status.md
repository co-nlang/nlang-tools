# nlang 引擎实现状态与规格差异分析

> 本文档对比 nlang-spec 规格书与 nlang-tools 引擎的当前实现状态。
> 规格书版本：量子化后（Heyting → 正交模格）
> 引擎版本：oo (Ouroboros)

---

## 1. 总览

| 规格章节 | 语义完整度 | 实现状态 | 关键差距 |
|:---------|:----------:|:--------:|:---------|
| SPEC_01 (格论基础) | 100% | **90%** | 正交模律验证、非分配性检测 |
| SPEC_06 (统一化逻辑) | 100% | **85%** | Bohrification 视角切换、局部分配律 |
| SPEC_09 (标准库) | 100% | **50%** | 创世预设值、Genesis 默认值 |
| SPEC_10 (演化与 Commit) | 100% | **70%** | #refine 精炼机制、权威签署验证 |
| SPEC_13 (OODP) | 100% | **10%** | LADD 引力路由、CAID 谱特征 |
| SPEC_17 (自我演化) | 100% | **0%** | N-1 自举算法、语义版本切换 |

---

## 2. 已实现功能详析

### 2.1 格论基础 (SPEC_01)

#### 已实现 ✓

| 功能 | 实现位置 | 状态 |
|:-----|:---------|:----:|
| Top `_` (万有子空间) | `value.rs:Value::Top` | ✓ |
| Bottom `_|_` (矛盾) | `value.rs:Value::Bottom` | ✓ |
| Meet `&` (合并/收敛) | `unify.rs:unify_internal` | ✓ |
| Join `|` (联集/叠加) | `eval.rs` | ✓ |
| Orthocomplement `!` | `complement.rs` | ✓ |
| 序位锚点 `#_`, `#_|_` | `value.rs:AtomKind::TagStart/TagEnd` | ✓ |
| 原子同构展开 | `unify.rs` (Atom → `{%val: atom}`) | ✓ |
| Cocoon 封闭世界 | `value.rs:ComboVal.closed` | ✓ |

#### 部分实现 ◐

| 功能 | 规格要求 | 当前实现 | 差距 |
|:-----|:---------|:---------|:-----|
| 正交模律验证 | $B = A \sqcup (B \sqcap !A)$ 当 $A \le B$ | 未实现自动验证 | 缺少几何自洽性检查 |
| 德摩根定律 | `!(A|B) = !A & !B` | 基本实现 | 联集补集需测试 |
| 非分配性检测 | 运算顺序影响语义 | 无检测 | 无警告机制 |

#### 未实现 ○

| 功能 | 说明 |
|:-----|:-----|
| 算术禁止 | `#_ + 1` 应返回 `_|_` (%cause: #arithmetic_on_anchor) |
| 区间定义 | `1..#_` 语法解析 |

---

### 2.2 统一化逻辑 (SPEC_06)

#### 已实现 ✓

| 规则 | 实现位置 | 说明 |
|:-----|:---------|:-----|
| 等值合并 | `unify.rs` | $A = B \Rightarrow C = A$ |
| Top 合并 | `unify.rs` | $A = \_ \Rightarrow C = B$ |
| Bottom 合并 | `unify.rs` | $A = \bot \Rightarrow C = \bot$ |
| 互斥原子 | `unify.rs` | 不同原子 → $\bot$ |
| 精确度优先 | `unify.rs` | #exact ⊂ #blur 吸收律 |
| Combo 递归收敛 | `unify.rs` | 按字段逐个合并 |
| Cocoon 封闭违规 | `unify.rs` | 未定义字段 → $\bot$ |
| 极小元素规则 | `dispatch.rs` | 模式匹配优先级 |
| 联集分支化简 | `eval.rs` | 冪等化简、空集消除 |

#### 部分实现 ◐

| 功能 | 差距 |
|:-----|:-----|
| 局部分配律 (Bohrification) | 无视角切换机制，无法检测交换性 |
| 保守性原则 | #blur 状态下未判定时，应保留两者 |

#### 未实现 ○

| 功能 | 说明 |
|:-----|:-----|
| 组合爆炸保护 | `%max_pattern_nodes` 检测与处理 |
| 策略切换 | #blur / #strict / #approximate 策略 |

---

### 2.3 标准库 (SPEC_09)

#### 已实现 ✓

| 模块 | 实现位置 | 功能 |
|:-----|:---------|:-----|
| **~%Math** | `builtins/math.rs` | `/add`, `/sub`, `/mul`, `/div`, `/rem`, `/abs`, `/pow`, `/bit*`, `/shl`, `/shr`, `/random` |
| **~%Complex** | `builtins/math.rs` | `/conj`, `/phase`, `/real`, `/imag` |
| **~%List** | `builtins/list.rs` | `/len`, `/at`, `/concat`, `/reverse`, `/slice`, `/zip`, `/sort`, `/map`, `/fold`, `/filter` |
| **~%Str** | `builtins/string.rs` | `/concat`, `/len`, `/trim`, `/split`, `/join`, `/replace`, `/to_lower`, `/to_upper`, `/starts_with`, `/ends_with`, `/contains` |
| **~%Cond** | `builtins/cond.rs` | `/if`, `/cond`, `/match` |
| **~%Time** | `builtins/time.rs` | `/now` |
| **~%Refl** | `builtins/reflection.rs` | `/keys`, `/has`, `/is_cocoon`, `/type_of` |
| **~%Engine** | `builtins/engine.rs` | `/observe`, `/save` |
| **~%Disc** | `builtins/disc.rs` | `/connect`, `/fetch`, `/identify` |

#### 部分实现 ◐

| 功能 | 规格要求 | 当前实现 |
|:-----|:---------|:---------|
| EML 算子 | `/eml(x, y) = exp(x) - ln(y)` 作为数学 LUCA | 未实现，各函数独立 |
| `/exp`, `/ln`, `/sin`, `/cos`, `/sqrt` | 由 EML 派生 | 未实现 |
| 代数介面 | `%fmap`, `%fold`, `%concat`, `%bind` | 仅态射实现，无元字段 |

#### 未实现 ○

| 功能 | 说明 |
|:-----|:-----|
| 创世预设值 | `%fuel: 10000`, `%max_branches: 64`, `%timeout: 1000` 等默认值 |
| 容器型别 | `@option`, `@result` 标准定义 |
| 分支切割处理 | `ln(0)` → #blur (%cause: #log_singularity) |
| `%branch` 元字段 | Riemann 面层级选择 |

---

### 2.4 演化与 Commit (SPEC_10)

#### 已实现 ✓

| 功能 | 实现位置 | 说明 |
|:-----|:---------|:-----|
| HEAD 指针 | `universe.rs` | 当前 Commit |
| Staged 暂存区 | `universe.rs` | 未固化定义 |
| Commit 链 (DAG) | `storage.rs` | 历史记录 |
| `/observe` | `builtins/engine.rs` | 观测路径 |
| `/evolve` | CLI `oo evolve` | 注入定义 |
| `/commit` | CLI `oo commit` | 固化快照 |
| CAID 计算 | `value.rs:content_hash()` | SHA256 |

#### 部分实现 ◐

| 功能 | 差距 |
|:-----|:-----|
| 乐观并发合并 | 有基本实现，无 `%cause` 冲突揭露 |
| 原子性保证 | 提交失败时无完整回滚 |

#### 未实现 ○

| 功能 | 说明 |
|:-----|:-----|
| **#refine 精炼机制** | 核心缺失：宣告模糊→精确的格论精炼 |
| 权威签署验证 | `%authority.signer` 验证 |
| 等价映射合成 | `~%Engine.equivalence_map` 动态视图 |
| 几何单调性验证 | $ID_{new} \sqsubseteq ID_{old}$ 判定 |

---

### 2.5 OODP 发现协议 (SPEC_13)

#### 已实现 ✓ (基础)

| 功能 | 实现位置 | 说明 |
|:-----|:---------|:-----|
| CAID 基础格式 | `value.rs` | SHA256 hash |
| Peer 注册 | `lib.rs:peers` | 本地/远程节点 |
| `/identify` | `builtins/disc.rs` | 计算 ContentHash |
| `/fetch` | `builtins/disc.rs` | 从节点获取内容 |
| `/connect` | `builtins/disc.rs` | 连接 TCP/本地 |

#### 未实现 ○ (核心差距)

| 功能 | 说明 | 规格章节 |
|:-----|:-----|:---------|
| **Lattice Sketch** | CAID 的谱特征摘要 (Base64) | REAL_03 §2.1 |
| **BN/ 序列化** | 位元级决定论规范化 | REAL_03 §4 |
| **LADD 引力路由** | 谜几何优化路由算法 | APP_05 §4 |
| **GPP 證明** | 几何概率零知识证明 | APP_05 §5 |
| **CIP 證明** | 因果完整性证明 | APP_05 §6 |
| **#refine 自动重定向** | 观测窗口内的 CAID 替换 | SPEC_13 §5.2 |
| **视界震盪防御** | 防语义日蚀攻击 | SPEC_13 §7.2 |
| **创世种子 CAID** | 标准库硬编码 CAID | SPEC_13 §3.1 |

---

### 2.6 自我演化 (SPEC_17)

#### 全部未实现 ○

| 功能 | 说明 |
|:-----|:-----|
| **N-1 自举算法** | 规格书 CAID 用上一版规则计算 |
| **双重身份锚定** | $ID_{phys}$ vs $ID_{logic}$ |
| **%promoter** | 迁移引导逻辑 |
| **退化封套布局** | 新规格封装为旧引擎原子 |
| **语义虚拟化挂载** | 版本切换执行环境 |
| **%compat 相容性宣告** | 版本兼容 CAID 集合 |

---

## 3. 量子化调整影响

规格书从 Heyting 代数（分配格）转变为**正交模格 (Orthomodular Lattice)**，这是核心数学模型的量子化。

### 3.1 对引擎的影响

| 項目 | Heyting (旧) | 正交模格 (新) | 实现调整 |
|:-----|:-------------|:--------------|:---------|
| 分配律 | 全域成立 | **仅局部成立** (Bohrification) | 需添加视角检测 |
| $A \sqcap (B \sqcup C)$ | $= (A \sqcap B) \sqcup (A \sqcap C)$ | **可能不同** | 需警告运算顺序 |
| 正交模律 | 无此概念 | $B = A \sqcup (B \sqcap !A)$ | 需实现验证 |
| CAID 计算 | 内容 hash | **谱特征 + 内容 hash** | 需实现 Lattice Sketch |

### 3.2 需新增的功能

1. **Bohrification 视角切换**：检测交换子代数，启用局部分配律
2. **正交模律验证**：几何自洽性检查
3. **谱特征提取**：投影算子特征值 → Lattice Sketch
4. **非分配性警告**：运算顺序可能影响结果时警告用户

---

## 4. OODP 协议调整 (LADD + CAID)

### 4.1 CAID 协议变化

| 组成部分 | 旧版 | 新版 (REAL_03) | 实现状态 |
|:---------|:-----|:---------------|:---------|
| 格式 | `hash:<algo>:<digest>` | `hash:<algo>:v<ver>:<sketch>:<digest>` | 未实现新格式 |
| Lattice Sketch | 无 | Base64 谱特征摘要 | 未实现 |
| 规范化 | JSON 序列化 | **BN/ 位元流** | 未实现 |
| 排序规则 | 无 | 前缀优先级 `% > ~% > ~ > @ > / > data` | 部分实现 |

### 4.2 LADD 协议 (APP_05)

LADD 是 OODP L3-L5 的谱几何优化扩展，当前完全未实现。

| 层次 | 功能 | 状态 |
|:-----|:-----|:----:|
| L3 收敛层 | 热带剪枝、分布式 `&` 运算 | ○ |
| L4 视角层 | Bohrification、权威格论 | ○ |
| L5 应用层 | `/find` 引力导航 | ○ |

---

## 5. 关键实现差距排序

### 高优先级 (核心功能)

| # | 功能 | 规格章节 | 影响 |
|:--|:-----|:---------|:-----|
| 1 | **BN/ 序列化** | REAL_03 | CAID 决定论基础 |
| 2 | **Lattice Sketch** | REAL_03 | CAID 新格式必需 |
| 3 | **创世预设值** | SPEC_09 | 标准库完整性 |
| 4 | **#refine 精炼机制** | SPEC_10, SPEC_13 | 版本演化核心 |

### 中优先级 (增强功能)

| # | 功能 | 规格章节 | 影响 |
|:--|:-----|:---------|:-----|
| 5 | EML 算子 | SPEC_09 | 数学 LUCA |
| 6 | 正交模律验证 | SPEC_01 | 几何自洽性 |
| 7 | LADD 基础路由 | APP_05 | 分布式发现 |
| 8 | 策略切换 | SPEC_06 | #blur/#strict |

### 低优先级 (未来功能)

| # | 功能 | 规格章节 | 影响 |
|:--|:-----|:---------|:-----|
| 9 | GPP/CIP 证明 | APP_05 | 零知识验证 |
| 10 | 自我演化 | SPEC_17 | 规格书自举 |
| 11 | 视界震盪防御 | SPEC_13 | 安全机制 |

---

## 6. 实现建议路径

### Phase 1: CAID 基础设施 (估计 2-3 周)

1. 实现 BN/ 序列化格式 (REAL_03 §4)
2. 添加 Lattice Sketch 计算 (REAL_03 §3.5)
3. 更新 CAID 格式为 `hash:<algo>:v1:<sketch>:<digest>`
4. 确定创世种子 CAID

### Phase 2: 标准库完善 (估计 1-2 周)

1. 补充创世预设值 (`%fuel`, `%max_branches`, 等)
2. 实现 EML 算子或其派生函数 (`/exp`, `/ln`, `/sin`, `/cos`, `/sqrt`)
3. 定义 `@option`, `@result` 标准型别
4. 完善代数介面 (`%fmap`, `%fold` 元字段)

### Phase 3: 精炼机制 (估计 2 周)

1. 实现 #refine Commit 类型
2. 等价映射合成算法
3. 权威签署验证
4. 观测窗口自动重定向

### Phase 4: LADD 基础 (估计 3-4 周)

1. 几何广告 (AdvertiseGeometry)
2. 几何查询 (DiscoverRequest)
3. 引力路由权重计算
4. 谜距离计算

---

## 7. 参考文件映射

| 规格章节 | 核心主题 | 引擎对应 |
|:---------|:---------|:---------|
| SPEC_01 | 正交模格 | `value.rs`, `unify.rs`, `complement.rs` |
| SPEC_06 | 统一化算法 | `unify.rs`, `dispatch.rs` |
| SPEC_09 | 标准库 | `builtins/*.rs` |
| SPEC_10 | 演化与 Commit | `universe.rs`, `storage.rs` |
| SPEC_13 | OODP | `builtins/disc.rs` (基础) |
| REAL_03 | CAID 协议 | `value.rs:content_hash()` (需扩展) |
| APP_05 | LADD | **未实现** |