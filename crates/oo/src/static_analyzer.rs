// static_analyzer.rs
// 靜態違規檢測器 - 實作 SPEC_15 反模式檢測（靜態部分）
//
// 注意：本檔的動態測試部分（run_single_test / TestResult / TestCase 等）在
// 先前版本從未被編譯（孤兒檔），且對到已過時的 interpreter API（ComboVal.fields /
// Value::Atom 單欄 / with_timeout）。nlint Tier 1 只用靜態分析部分（handover §6
// 非目標：不做求值／不依賴 interpreter crate 的 eval），動態部分已移除。

use nlang_parser::ast::{AtomKind, Expr, ExprKind, Field, FieldKey, Path, PathAnchor};
use std::collections::HashSet;

/// 靜態違規類型
#[derive(Debug, Clone, PartialEq)]
pub enum StaticViolation {
    /// 隨機性注入 (~%Math./random 等)
    RandomnessInjection { path: String, line: usize },
    /// 隱性環境依賴 (~%Time./now 等)
    EnvironmentDependency { path: String, line: usize },
    /// 跨路徑私有存取 (~ 欄位)
    PrivateAccessViolation { path: String, line: usize },
    /// 潛在無限遞迴
    PotentialInfiniteRecursion { name: String, line: usize },
    /// 型別衝突 (編譯期可檢測的)
    TypeConflict { message: String, line: usize },
}

impl StaticViolation {
    pub fn line(&self) -> usize {
        match self {
            StaticViolation::RandomnessInjection { line, .. } => *line,
            StaticViolation::EnvironmentDependency { line, .. } => *line,
            StaticViolation::PrivateAccessViolation { line, .. } => *line,
            StaticViolation::PotentialInfiniteRecursion { line, .. } => *line,
            StaticViolation::TypeConflict { line, .. } => *line,
        }
    }

    pub fn message(&self) -> String {
        match self {
            StaticViolation::RandomnessInjection { path, .. } => {
                format!("隨機性注入檢測: {} - 違反 Invariant 1 (決定論)", path)
            }
            StaticViolation::EnvironmentDependency { path, .. } => {
                format!("隱性環境依賴檢測: {} - 違反 Invariant 1 (決定論)", path)
            }
            StaticViolation::PrivateAccessViolation { path, .. } => {
                format!("私有欄位存取違規: {} - 違反 Invariant 3 (觀測純粹性)", path)
            }
            StaticViolation::PotentialInfiniteRecursion { name, .. } => {
                format!(
                    "潛在無限遞迴: {} - 建議添加終止條件或 %termination_proof",
                    name
                )
            }
            StaticViolation::TypeConflict { message, .. } => {
                format!("型別衝突: {}", message)
            }
        }
    }
}

/// 簡化型別表示
#[derive(Debug, Clone, PartialEq)]
pub enum SimpleType {
    Top,
    Bottom,
    Int,
    Float,
    Str,
    Tag(String),
    Combo(Vec<(String, SimpleType)>),
    Morphism(Box<SimpleType>, Box<SimpleType>), // param -> return
    Unknown,
}

/// 靜態分析器
pub struct StaticAnalyzer {
    violations: Vec<StaticViolation>,
    /// 已檢測的非確定性函數
    nondeterministic_fns: HashSet<String>,
    /// 已檢測的環境依賴函數
    env_dependency_fns: HashSet<String>,
    /// 型別環境：變數名 -> 型別
    type_env: std::collections::HashMap<String, SimpleType>,
}

impl StaticAnalyzer {
    pub fn new() -> Self {
        let mut nondeterministic_fns = HashSet::new();
        nondeterministic_fns.insert("~%Math./random".to_string());
        nondeterministic_fns.insert("~%Math./random_int".to_string());

        let mut env_dependency_fns = HashSet::new();
        env_dependency_fns.insert("~%Time./now".to_string());
        env_dependency_fns.insert("~%Time./today".to_string());
        env_dependency_fns.insert("~%Env./get".to_string());

        Self {
            violations: Vec::new(),
            nondeterministic_fns,
            env_dependency_fns,
            type_env: std::collections::HashMap::new(),
        }
    }

    /// 推斷表達式的型別
    fn infer_type(&self, expr: &Expr) -> SimpleType {
        match &expr.kind {
            ExprKind::Atom(atom) => match atom {
                AtomKind::Int(_) => SimpleType::Int,
                AtomKind::Float(_) => SimpleType::Float,
                AtomKind::Str(_) | AtomKind::MultilineStr(_) => SimpleType::Str,
                AtomKind::Tag(t) => SimpleType::Tag(t.clone()),
                AtomKind::Unit => SimpleType::Combo(vec![]),
                AtomKind::Top => SimpleType::Top,
                AtomKind::Bottom => SimpleType::Bottom,
                _ => SimpleType::Unknown,
            },
            ExprKind::Path(p) => {
                let path_str = path_to_string(p);
                self.type_env
                    .get(&path_str)
                    .cloned()
                    .unwrap_or(SimpleType::Unknown)
            }
            ExprKind::Morphism { .. } => {
                // 簡化：morphism 類型暫時標記為 Unknown -> Unknown
                SimpleType::Morphism(Box::new(SimpleType::Unknown), Box::new(SimpleType::Unknown))
            }
            ExprKind::Combo { fields, .. } => {
                let field_types: Vec<(String, SimpleType)> = fields
                    .iter()
                    .map(|f| {
                        let name = match &f.key {
                            FieldKey::Named { name, .. } => name.clone(),
                            FieldKey::Path(p) => p.segments.join("."),
                            _ => "unknown".to_string(),
                        };
                        (name, self.infer_type(&f.value))
                    })
                    .collect();
                SimpleType::Combo(field_types)
            }
            ExprKind::List(_) => SimpleType::Unknown, // 列表類型暫不詳細推斷
            _ => SimpleType::Unknown,
        }
    }

    /// 檢查型別衝突
    fn check_type_conflict(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Add(lhs, rhs)
            | ExprKind::Sub(lhs, rhs)
            | ExprKind::Mul(lhs, rhs)
            | ExprKind::Div(lhs, rhs)
            | ExprKind::Rem(lhs, rhs) => {
                let left_type = self.infer_type(lhs);
                let right_type = self.infer_type(rhs);

                // 檢查算術運算的型別
                match (&left_type, &right_type) {
                    (SimpleType::Int, SimpleType::Int) => {}
                    (SimpleType::Float, SimpleType::Float) => {}
                    (SimpleType::Int, SimpleType::Float) | (SimpleType::Float, SimpleType::Int) => {
                    }
                    (SimpleType::Str, _) | (_, SimpleType::Str) => {
                        self.violations.push(StaticViolation::TypeConflict {
                            message: format!(
                                "Cannot perform arithmetic on string types: {:?} + {:?}",
                                left_type, right_type
                            ),
                            line: expr.span.start,
                        });
                    }
                    (SimpleType::Combo(_), _) | (_, SimpleType::Combo(_)) => {
                        self.violations.push(StaticViolation::TypeConflict {
                            message: "Cannot perform arithmetic on Combo types".to_string(),
                            line: expr.span.start,
                        });
                    }
                    _ => {}
                }
            }
            ExprKind::Apply(f, arg) => {
                let func_type = self.infer_type(f);
                let arg_type = self.infer_type(arg);

                // 檢查是否對非函數進行應用
                match &func_type {
                    SimpleType::Morphism(_, _) => {}
                    SimpleType::Unknown => {}
                    _ => {
                        self.violations.push(StaticViolation::TypeConflict {
                            message: format!(
                                "Cannot apply non-function type {:?} to argument {:?}",
                                func_type, arg_type
                            ),
                            line: expr.span.start,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    /// 分析程式，返回違規列表
    pub fn analyze(&mut self, fields: &[Field]) -> Vec<StaticViolation> {
        self.violations.clear();

        for field in fields {
            self.analyze_field(field);
        }

        self.violations.clone()
    }

    fn analyze_field(&mut self, field: &Field) {
        // 檢查欄位名是否為測試模式
        let is_test = match &field.key {
            FieldKey::Named { name, .. } => name.starts_with("test_") || name.starts_with("~%test"),
            _ => false,
        };

        // 分析欄位值
        self.analyze_expr(&field.value, is_test);
    }

    fn analyze_expr(&mut self, expr: &Expr, in_test: bool) {
        let line = expr.span.start;

        // 先檢查型別衝突
        self.check_type_conflict(expr);

        match &expr.kind {
            ExprKind::Path(path) => {
                self.check_path_violations(path, line);
            }
            ExprKind::Apply(f, arg) => {
                self.analyze_expr(f, in_test);
                self.analyze_expr(arg, in_test);

                // 檢查函數應用是否為非確定性
                if let ExprKind::Path(p) = &f.kind {
                    let path_str = path_to_string(p);
                    if self.nondeterministic_fns.contains(&path_str) {
                        self.violations.push(StaticViolation::RandomnessInjection {
                            path: path_str.clone(),
                            line,
                        });
                    }
                    if self.env_dependency_fns.contains(&path_str) {
                        self.violations
                            .push(StaticViolation::EnvironmentDependency {
                                path: path_str.clone(),
                                line,
                            });
                    }
                }
            }
            ExprKind::Pipe(lhs, rhs) => {
                self.analyze_expr(lhs, in_test);
                self.analyze_expr(rhs, in_test);
            }
            ExprKind::Morphism { param, body } => {
                self.analyze_expr(param, in_test);
                self.analyze_expr(body, in_test);

                // 檢查遞迴終止
                self.check_termination(param, body);
            }
            ExprKind::Meet(a, b) | ExprKind::Join(a, b) | ExprKind::Diff(a, b) => {
                self.analyze_expr(a, in_test);
                self.analyze_expr(b, in_test);
            }
            ExprKind::Add(a, b)
            | ExprKind::Sub(a, b)
            | ExprKind::Mul(a, b)
            | ExprKind::Div(a, b)
            | ExprKind::Rem(a, b) => {
                self.analyze_expr(a, in_test);
                self.analyze_expr(b, in_test);
            }
            ExprKind::Eq(a, b)
            | ExprKind::Ne(a, b)
            | ExprKind::Lt(a, b)
            | ExprKind::Gt(a, b)
            | ExprKind::Lte(a, b)
            | ExprKind::Gte(a, b) => {
                self.analyze_expr(a, in_test);
                self.analyze_expr(b, in_test);
            }
            ExprKind::Combo { fields, .. } => {
                for f in fields {
                    self.analyze_field(f);
                }
            }
            ExprKind::List(items) => {
                for item in items {
                    self.analyze_expr(item, in_test);
                }
            }
            ExprKind::Ternary {
                cond,
                then_branch,
                else_branch,
            } => {
                self.analyze_expr(cond, in_test);
                self.analyze_expr(then_branch, in_test);
                self.analyze_expr(else_branch, in_test);
            }
            ExprKind::Unary { expr, .. } => {
                self.analyze_expr(expr, in_test);
            }
            ExprKind::Lens(obj, key) => {
                self.analyze_expr(obj, in_test);
                self.analyze_expr(key, in_test);
            }
            ExprKind::TypeAnnotation(expr, ty) => {
                self.analyze_expr(expr, in_test);
                self.analyze_expr(ty, in_test);
            }
            ExprKind::AnonSet(expr)
            | ExprKind::Spread(expr)
            | ExprKind::Complement(expr)
            | ExprKind::Structural(expr) => {
                self.analyze_expr(expr, in_test);
            }
            ExprKind::Range { start, end, step } => {
                self.analyze_expr(start, in_test);
                self.analyze_expr(end, in_test);
                if let Some(s) = step {
                    self.analyze_expr(s, in_test);
                }
            }
            ExprKind::Tuple(items) => {
                for item in items {
                    self.analyze_expr(item, in_test);
                }
            }
            ExprKind::LatticeEq(a, b) | ExprKind::Probe(a, b) => {
                self.analyze_expr(a, in_test);
                self.analyze_expr(b, in_test);
            }

            ExprKind::Interpolated(parts) => {
                for part in parts {
                    if let nlang_parser::ast::StringPart::Interpolated(e) = part {
                        self.analyze_expr(e, in_test);
                    }
                }
            }
            _ => {}
        }
    }

    fn check_path_violations(&mut self, path: &Path, line: usize) {
        let path_str = path_to_string(path);

        // 檢查私有欄位存取 (~)
        if path_str.contains("~") && !path_str.starts_with("~%") {
            // 簡單檢查：如果路徑包含 ~ 但不是 ~% 開頭，可能是違規
            // 實際上需要更複雜的語境分析
            if path.anchor != PathAnchor::Bare || !path.segments.is_empty() {
                self.violations
                    .push(StaticViolation::PrivateAccessViolation {
                        path: path_str.clone(),
                        line,
                    });
            }
        }

        // 檢查非確定性函數
        if self.nondeterministic_fns.contains(&path_str) {
            self.violations.push(StaticViolation::RandomnessInjection {
                path: path_str.clone(),
                line,
            });
        }

        // 檢查環境依賴
        if self.env_dependency_fns.contains(&path_str) {
            self.violations
                .push(StaticViolation::EnvironmentDependency {
                    path: path_str.clone(),
                    line,
                });
        }
    }

    fn check_termination(&mut self, param: &Expr, body: &Expr) {
        // 檢查 morphism body 中是否有遞迴調用模式
        // 提取參數名
        let param_name = self.extract_param_name(param);

        // 檢查 body 中是否有對參數的自應用 (param param)
        self.check_expr_for_self_application(body, &param_name, 0);
    }

    fn extract_param_name(&self, param: &Expr) -> String {
        match &param.kind {
            ExprKind::Path(p) if p.segments.len() == 1 => p.segments[0].clone(),
            ExprKind::Atom(AtomKind::Tag(t)) => t.clone(),
            _ => "_".to_string(),
        }
    }

    fn check_expr_for_self_application(&mut self, expr: &Expr, param_name: &str, depth: usize) {
        if depth > 10 {
            return;
        }

        match &expr.kind {
            ExprKind::Apply(f, arg) => {
                // 檢查是否是 self-application (param param)
                if self.is_param_reference(f, param_name)
                    && self.is_param_reference(arg, param_name)
                {
                    self.violations
                        .push(StaticViolation::PotentialInfiniteRecursion {
                            name: format!("{} {}", param_name, param_name),
                            line: expr.span.start,
                        });
                }
                self.check_expr_for_self_application(f, param_name, depth + 1);
                self.check_expr_for_self_application(arg, param_name, depth + 1);
            }
            ExprKind::Morphism {
                param,
                body: m_body,
                ..
            } => {
                let new_param = self.extract_param_name(param);
                self.check_expr_for_self_application(m_body, &new_param, depth + 1);
            }
            ExprKind::Pipe(lhs, rhs) => {
                self.check_expr_for_self_application(lhs, param_name, depth + 1);
                self.check_expr_for_self_application(rhs, param_name, depth + 1);
            }
            ExprKind::Meet(a, b) | ExprKind::Join(a, b) | ExprKind::Diff(a, b) => {
                self.check_expr_for_self_application(a, param_name, depth + 1);
                self.check_expr_for_self_application(b, param_name, depth + 1);
            }
            ExprKind::Ternary {
                cond,
                then_branch,
                else_branch,
            } => {
                self.check_expr_for_self_application(cond, param_name, depth + 1);
                self.check_expr_for_self_application(then_branch, param_name, depth + 1);
                self.check_expr_for_self_application(else_branch, param_name, depth + 1);
            }
            ExprKind::Combo { fields, .. } => {
                for field in fields {
                    self.check_expr_for_self_application(&field.value, param_name, depth + 1);
                }
            }
            ExprKind::List(items) => {
                for item in items {
                    self.check_expr_for_self_application(item, param_name, depth + 1);
                }
            }
            _ => {}
        }
    }

    fn is_param_reference(&self, expr: &Expr, param_name: &str) -> bool {
        match &expr.kind {
            ExprKind::Path(p) if p.segments.len() == 1 && p.segments[0] == param_name => true,
            ExprKind::Atom(AtomKind::Tag(t)) if t == param_name => true,
            _ => false,
        }
    }
}

fn path_to_string(path: &Path) -> String {
    let anchor = match path.anchor {
        PathAnchor::Root => "_.",
        PathAnchor::Bare => "",
        PathAnchor::Parent(n) => return format!("{}.", "^".repeat(n as usize)),
        PathAnchor::Current => "~.",
    };

    let segments = path.segments.join(".");
    format!("{}{}", anchor, segments)
}

/// 測試結果（靜態部分）
#[derive(Debug)]
pub struct TestResult {
    pub file: String,
    pub tests_run: usize,
    pub tests_passed: usize,
    pub violations: Vec<StaticViolation>,
}

impl TestResult {
    pub fn success(&self) -> bool {
        self.violations.is_empty()
    }
}

/// 執行靜態測試（純靜態分析，不做求值）
pub fn run_static_tests(files: &[std::path::PathBuf], pattern: Option<&str>) -> Vec<TestResult> {
    let mut results = Vec::new();

    for file in files {
        if let Some(pat) = pattern {
            if !file.to_string_lossy().contains(pat) {
                continue;
            }
        }

        let content = std::fs::read_to_string(file).unwrap_or_default();
        let program = nlang_parser::parse_program(&content);

        let result = match program {
            Ok(prog) => {
                let mut analyzer = StaticAnalyzer::new();
                let violations = analyzer.analyze(&prog.fields);

                // 計算測試數量（~%test 或 test_ 開頭的欄位）
                let test_count = prog
                    .fields
                    .iter()
                    .filter(|f| match &f.key {
                        FieldKey::Named { name, .. } => {
                            name.starts_with("test_") || name.starts_with("~%test")
                        }
                        _ => false,
                    })
                    .count();

                TestResult {
                    file: file.to_string_lossy().to_string(),
                    tests_run: test_count,
                    tests_passed: if violations.is_empty() { test_count } else { 0 },
                    violations,
                }
            }
            Err(e) => TestResult {
                file: file.to_string_lossy().to_string(),
                tests_run: 0,
                tests_passed: 0,
                violations: vec![StaticViolation::TypeConflict {
                    message: format!("Parse error: {}", e),
                    line: 0,
                }],
            },
        };

        results.push(result);
    }

    results
}
