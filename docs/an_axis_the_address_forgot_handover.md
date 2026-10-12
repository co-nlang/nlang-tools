# 工單：位址忘了的軸（Q-083）

> 佇列 `nlang-spec/meta/WORK_QUEUE.md` Q-083（原 Inbox `interrupt-candidate`「位址不含欄位的軸」）／**裁定 D106**（用戶 2026-10-12：甲 逐條目一個軸位元組；乙 照 O63 走格式閘，舊倉照讀；不與 O88 併）／依據 `REAL_03` §6.6（讀時重算位址）、§6.7 第二條 MUST NOT（`=` 分得開的兩個值，位址必須相異）、§6.8.1（容器宣告編碼）、O63（位址規則一改，舊倉在容器宣告上被辨識並以舊規則讀）
> 探針（已預先提交並校準）：`crates/oo/tests/an_axis_the_address_forgot_probe_test.rs`（11）、`crates/interpreter/tests/an_axis_the_address_forgot_union_probe_test.rs`（2）、`crates/interpreter/tests/an_axis_the_address_forgot_epochs_probe_test.rs`（4）；夾具 `crates/oo/tests/fixtures/epoch1_repo`（真 `oo v0.84.0` 所建）
> 基線：dev `53796d2`／`oo v0.84.0` ⟹ CLI 探針 **5 綠 6 紅**、union **0／2**、epochs **編譯失敗**（它指名的 API 不存在）；紅的全紅在一般斷言。
> **這是一次紀元：每一個位址都會動。** 本工單的大半篇幅是「舊的怎麼辦」。

## 1. 缺陷

〔讀〕`crates/interpreter/src/bn_serial.rs` `serialize_combo` 以 `(軸階, 鍵名)` 排序條目，**寫進位元組的只有鍵名與值**——軸階不入位元組流（CAID v2 誕生時即如此）。`REAL_03` §6.3 的範例逐字把這件事寫成規格（B4，2026-07-19：「軸階只參與排序，不寫入位元組流」），與 §6.7 第二條（D69）自相矛盾。**任何照 §6.3 寫的實作都中。**

〔量，v0.84.0〕提交後的根：`t: { int: 1 }`／`t: { @int: 1 }`／`t: { ~int: 1 }` **三者同為** `4916104e…`；`t: { m: 1 }`／`t: { %m: 1 }` 同 `7bf8c6d5…`；`t: { f: (x -> x) }`／`t: { /f: (x -> x) }` 同 `8ef73b80…`；`t: { h: 3 }`／`t: { ~h: 3 }` 同 `06546026…`。值層同樣：`content_digest` 對 `{ int: 1 }` 與 `{ @int: 1 }` 皆 `7aa311aa…`、對 `{ m: 1 }` 與 `{ %m: 1 }` 皆 `bdb167fc…`。`=` 判每一對 `#false`。**三種靜默錯答案**：

*   **合一**以位址相等早退：`{ int: 1 } & { @int: 1 }` ⟹ `{ int: 1 }`，反序 ⟹ `{ @int: 1 }`——不交換、一個欄位無聲消失；`(a & b) = (b & a)` ⟹ `#false`。
*   **`--pin`**：已提交 `t: { int: 1 }`，`evolve --pin --grant pin` 換成 `t: { @int: 1 }`、`commit --grant pin` 回報成功，`_.t` 仍讀 `{ int: 1 }`（新根與舊根同址，物件已存在）。
*   **分派**：分支以模式的位址命名（`dispatch.rs` `pattern_branch_name`），`{ @{ { m: 1 } }: "a", @{ { %m: 1 } }: "b" }` 只剩**一個**分支（本體 `"b" & "a"`），施於 `{ m: 1 }` ⟹ ⊥ `#conflict`；對照 `%m: 2` 兩個分支、答 `"a" | "b"`。

〔讀〕**同一類的第二處：`serialize_union` 沒有標記也沒有計數**，聯集的位元組就是成員位元組接在一起——身分編碼因此不是前綴碼。值層見證：`Value::Range` 以 `(1 | 2)` 為下界、`3` 為上界，與以 `1` 為下界、`(2 | 3)` 為上界，位元組逐字相同（`18 [12 01][12 02][12 03] 00`）。〔量〕表層程式沒找到見證：上下界是聯集的區間在求值時先塌成 ⊥。⟹ 不是活缺陷，而是離活缺陷一個引數。

## 2. 裁定與依據

*   **D106 甲：每個條目寫一個軸位元組。** 軸位元組＝`REAL_03` §6.2 的優先級：system `1`、meta `2`、type `3`、logic `4`、data `5`、local `6`；`7`（`~@`）與 `8`（`~/`）**保留**，今日引擎不產生（見 §6）。`legacy_fields` 記為 `5`、`legacy_local` 記為 `6`（與今日的排序鍵相同）。
*   **D106 乙：舊倉照讀（O63 的格式閘）。** v0.84.0 的身分編碼凍結，**只用於驗證既有的位址**與辨認舊標準根；**新寫入一律用新編碼**。
*   **D106 丙：不與 O88（浮點數）併。**
*   **驗收方校準時定的四件事**（屬於 D106 的射程，用戶可推翻；若你認為哪一件錯了，寫在 8.3，不要自行改）：
    1.  **Union 同一次修**：`Union ::= 0x05 [成員數: LEB128] 成員*`，成員依各自的位元組排序、去重。理由：與軸是同一類（編碼不是前綴碼），而這次紀元本來就移動每一個位址。
    2.  **紀元位元組（域分離）**：身分雜湊的輸入是 `0x80 ‖ 值的位元組`；巢狀的值不加。理由：讀路徑「新編碼不中再試凍結編碼」只在兩個紀元的原像集合不相交時才健全——否則一個新位址可能被另一個值的舊編碼滿足。`0x80` 不是任何紀元一位元組串的首位元組（紀元一的首位元組只會是值標記 `0x01–0x04`、`0x10–0x21`、`0xFD–0xFF`）。**代價：每一個值的位址都動，連原子也動**（`0` 由 `08da7c45…` 變了）。另一條路是只給 Combo／Union 換新標記，原子位址不動；驗收方取紀元位元組，理由是它把「身分編碼的版本」放進了雜湊原像而不是物件裡（§6.8.1 不禁），下一次紀元用 `0x81` 即可，不必再找新標記。
    3.  **讀寫規則**：讀——先以新編碼重算，不中再以凍結編碼重算，兩者皆不中才是 `#caid_mismatch`；寫——一律新編碼。**一個宣告 `encoding=5` 的倉，在本引擎寫入第一個物件之前，先把 `objects.format` 升為 `encoding=6`**（v0.84.0 只認 1–5，於是在閘上具名拒絕，而不是把新物件說成 corrupt）；本引擎新建的倉宣告 `encoding=6`。只讀的指令不得改宣告。
    4.  **排序照今日引擎**：先軸位元組、再鍵名的 UTF-8 位元組序。`REAL_03` §6.2「純數字鍵排最後」從未被實作，規格收尾時刪除（驗收方的事）。
*   **新的身分編碼（在 `REAL_03` 改寫之前，以本節為準；r6 與 e1 以它為儀器）**：
    ```
    digest      = SHA-256( 0x80 ‖ V(value) )
    V(Combo)    = [0x01 開放 | 0x02 繭] [條目數: LEB128] ( [軸: u8] [鍵: LEB128 長度 + UTF-8] V(值) )*
                  條目依 (軸, 鍵的 UTF-8 位元組) 遞增
    V(Union)    = 0x05 [成員數: LEB128] V(成員)*     成員依 V(成員) 的位元組遞增、去重
    其餘        = 與 v0.84.0 相同
    lattice sketch：與 v0.84.0 同一演算法，逐欄位的振幅取該欄位值的 V(·)（不含 0x80）
    ```
*   **凍結編碼（紀元一）**＝v0.84.0 的 `serialize_bn`／`content_digest`，**逐位元組**；`e1` 以 v0.84.0 自己算出的十個摘要為準。

## 3. 射程＝不變式（不是機制）

*   **I1 `=` 分得開的兩個值，位址不同**——在本引擎新建的倉裡，以及在記憶體裡（合一的早退、分派分支的名字、任何以位址做捷徑之處）（r1、r2、r4、union r1／r2）。**軸與 Union 是今天找到的兩處；「身分編碼是前綴碼」才是要守的不變式。** 若你找到別的構造位元組不夠分辨（本卡偵察只讀了 Combo 與 Union），一併在射程內，在 8.1 列出。
*   **I2 `--pin` 換軸會落地**（r3）。
*   **I3 紀元一的倉照讀、照寫、照驗**：打得開、答案與 v0.84.0 相同、提交後歷史完整、竄改仍被抓到、`gc` 不刪可達物件（g1–g4）；樹裡既有的舊倉測試（`encoding4_repo`、`layout2_framed_repo`、`layout5*`、`layout6_signed_repo`、`layout7_repo`、`pre_sentinel_repo` 相關的全部）保持綠。
*   **I4 兩個紀元的位址空間不相交，而凍結編碼是 v0.84.0 的**（epochs e1–e4）。`bn_serial::with_legacy_identity(f)`（以凍結編碼執行 `f`，離開時——含展開——還原）**是本工單指定的 API**；怎麼實作由你。
*   **I5 寫過的倉自己說**：本引擎寫入過的 `encoding=5` 倉宣告 `encoding=6`；只讀不改（r5）。
*   **I6 新的身分紅線**＝§2 的編碼（r6）。**既有測試裡的身分字面值改成新值**——只限下面〔附錄〕列的那些測試、只限字面的摘要與 `encoding=5` 字樣；改動處的註解寫明「Q-083／D106 紀元」；**新值必須是你的引擎算出來的，而 r6 會檢查它與驗收方的參考編碼一致。** 其他任何既有測試的修改都要在 8.2 逐項指名並說理由。

## 4. 紅線（今天綠，必須保持綠）

*   **除〔附錄〕所列之外的每一支既有測試**；conformance **162／162**。
*   **交叉編譯** `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` **0 error**。
*   `~%Math./add` (1, 2)=3、(1, 3)=4，且不建 `.oo/`；新倉 `layout=9`（佈局不動）。
*   `pin_probe_test` 15／15（驗收方已把它的儀器從子字串 `"42"` 收緊成 `"x: 42"`：新標準根的位址 `…898e42cb…` 恰含 `42`，那兩支在紀元之後會因為一個摘要而不是一個值而紅——機率型的衝突）。

## 5. 必答（請在交付報告裡回答，逐題）

*   **Q1** 每一處計算位址的地方，分兩類列出：**倉的位址**（寫入、讀取驗證、`gc`、對等點、標準根架、提交）與**記憶體裡的身分**（合一早退、分派分支名、memo／cycle key、`disc.identify`……）。哪些會走凍結編碼、為什麼；有沒有哪一處把讀進來的舊物件的**舊位址**又當成它**現在**的身分用（例如拿 HEAD 的舊根位址去跟一個新算的位址比「有沒有變」）。
*   **Q2**〔量〕用 `/home/gali/nlang-baselines/v0.84.0-verify/target/release/oo` 開 (a) 本引擎只讀過的紀元一倉、(b) 本引擎寫過一次的紀元一倉、(c) 本引擎新建的倉：各得什麼（逐字）。
*   **Q3**〔量，陳述〕**殘留的弱點**：紀元一的位址本來就分不開 `{ int: 1 }` 與 `{ @int: 1 }`。在紀元一倉裡把根物件裡的 `int: 1` 改成 `@int: 1`，本引擎讀那個舊位址得什麼？對等點 `fetch` 一個紀元一位址呢？**不要求修**（那是紀元一位址承諾得比看起來少，不是本引擎的讀法錯），只要求如實陳述。
*   **Q4**〔量〕紀元範圍：下列各得什麼新位址、有沒有任何一個保住了舊位址——§4 的五個根與標準根、`0`、`"a"`、`{}`、`{ a: 1 }`、一個只含箭頭的根、一個 `#blur`。
*   **Q5**〔量〕`oo migrate` 對 `encoding=4` 與 `encoding=5` 的倉現在做什麼；它移不移動任何位址；它印的「鎖掉哪些引擎」對不對（`encoding=6` 之後 v0.84.0 以前全部打不開）。
*   **Q6** 標準根架（`shipped_standard_roots`）現在認得哪些摘要（列出），每一個是哪個歷史形 × 哪個紀元。

## 6. 明文不在射程內

*   **O88 浮點數**（D106 丙）。
*   **`~@` 與 `~/` 被無聲丟掉**：`SPEC_14` 文法與 `REAL_03` §6.2 都列了隱藏型別與隱藏邏輯，〔量，v0.84.0〕`{ ~@int: 1 }` 求值得 `{}`、`t: { ~@int: 1 }` 提交 rc=0 而什麼都沒存。另列 Inbox。軸位元組 7、8 為它們保留。
*   **標籤鍵 `#a:` 與引號鍵 `"#a":` 同址**（`t: { #a: 1 }`／`t: { "#a": 1 }` 同為 `cbeedbbe…`）：Inbox 既有列，那是剖析層把兩者讀成同一個鍵，不是位元組的事；本卡之後仍會同址。
*   `legacy_fields` 與 `data` 同名時兩條同軸條目（M7 陷阱，今日無建構路徑）。
*   規格文字（`REAL_03` §6.2／§6.3 改寫、§2 的紀元說明）、`CHANGELOG`、`GENESIS_CAID` 的處理：驗收方收尾。

## 7. 探針完整性（沒有 `#[ignore]`：紅就是紅）

*   紅探針（CLI r1–r6、union r1–r2、epochs e1–e4）基線必須是紅的；**你可以讓它們變綠，不得改寫它們的宣稱。** 守衛（g1–g5）今天綠。`rustfmt` 不得掃探針檔。夾具一個位元組都不得改。
*   探針的 `commit` 輔助函式斷言 evolve 與 commit 的離開碼皆為 0。
*   〔驗收方校準〕**兩極皆做**：
    基線 v0.84.0：CLI **5 綠 6 紅**（g1–g5 綠；r1–r6 紅，皆在一般斷言）；union **0／2**（斷言）；epochs **編譯失敗**（`with_legacy_identity` 不存在——這是它在基線上唯一能有的紅法）。
    一個參考實作（`bn_serial`、`value.rs` 的位址快取、`storage.rs` 讀寫、`gc.rs`、`oodp.rs`、標準根架；6 檔，+118／−22）⟹ CLI **11／11**、union **2／2**、epochs **4／4**、`pin_probe` **15／15**。
    同一參考實作跑全樹（未含新探針）：**276 target／2570 passed／56 failed**——56 支全部是〔附錄〕的身分字面值與 `encoding=5` 字樣；**零支是舊倉讀不開**。
    **對照：一個「只換編碼、不管舊倉」的天真實作**（逐條目軸位元組，沒有凍結編碼）跑全樹 **2546 passed／80 failed**：除了同樣的字面值，**另有 24 支舊倉測試讀不開**（`#caid_mismatch … corrupt`——一句不實的完整性指控——或「標準根 `7038e250…` 不可得」），v0.84.0 建的倉連 `oo log` 都 rc=1。I3 擋的就是它。
    **守衛的變異**（每一個只換一處；參考實作上量，量完還原）：凍結編碼變成空操作 ⟹ g1、g2、g3、g4、r5、e1、e2、e3；拿掉紀元位元組 ⟹ e2、r6；寫入不升宣告 ⟹ r5。
    〔驗收方自陳〕(1) r4 初版把 `{ %m: 1 }` 也拿同一張對照表比，參考實作上紅——對照表 `%m: 2` 的那個模式不收 `%m: 1`，是儀器錯；已改成各配各的引數。(2) g2 初版用裸 64 字元摘要 `inspect`，v0.84.0 上就紅（`Invalid CAID format`）——守衛在基線上紅就不是守衛；已改成從 `log` 取完整位址。(3) `pin_probe_test` 的 `"42"` 見 §4。

〔附錄〕**身分字面值要改的既有測試**（參考實作上的 56 支；你的實作上若多出或少了，逐支在 8.2 說明）：

*   `a_commit_that_ate_what_it_never_read_probe_test`：`g1_identity_is_a_red_line`
*   `a_commit_that_closes_the_door_probe_test`：`g3_the_identity_of_a_current_repo_does_not_move`、`g4_an_encoding_3_store_keeps_its_sentinel`
*   `a_commit_that_is_a_value_probe_test`：`g4_value_addresses_do_not_move`、`g5_identify_is_not_changed_by_this_arc`
*   `a_contract_that_lives_in_rust_probe_test`：`g5_identity_does_not_move`
*   `a_definition_that_would_not_merge_probe_test`：`g1_a_lone_morphism_keeps_its_root`、`g2_tables_meet_as_before`
*   `a_history_written_twice_probe_test`：`g2_identity_is_a_red_line`
*   `a_name_spread_in_probe_test`：`g3_unchanged_values_keep_their_root`
*   `a_private_never_committed_probe_test`：`g2_other_roots_do_not_move`
*   `a_record_that_agrees_with_itself_probe_test`：`g4_identity_is_a_red_line`
*   `a_refusal_written_as_an_answer_probe_test`：`r9_a_half_migrated_store_can_still_be_completed`
*   `a_store_written_in_another_language_probe_test`：`g1_the_root_address_does_not_move`、`g2_the_standard_root_digest_does_not_move`、`r1_the_root_object_is_not_a_rust_enum_dump`、`r2_the_standard_root_is_not_a_hex_blob`、`r6_a_new_store_declares_the_new_encoding`、`r7_an_encoding_4_store_opens_and_migrates_without_moving_anything`
*   `a_success_that_was_a_bottom_probe_test`：`g1_a_clean_commit_keeps_its_identity`
*   `a_table_that_forgot_its_patterns_probe_test`：`g4_bare_binders_keep_their_bytes`
*   `a_table_that_lost_its_patterns_probe_test`：`g1_roots_without_a_pattern_key_stay`
*   `a_table_you_cannot_read_probe_test`：`g3_a_plain_combo_is_not_a_table`
*   `a_type_you_could_not_carry_probe_test`：`g3_no_address_moves`
*   `a_union_that_could_not_be_applied_probe_test`：`g4_union_values_and_atoms`
*   `a_working_set_that_is_a_set_probe_test`：`g4_identity_is_a_red_line`
*   `an_address_that_depends_on_a_printer_probe_test`：`g4_a_universe_without_a_thunk_does_not_move`
*   `an_authority_you_could_delete_probe_test`：`g1_identity_is_a_red_line`
*   `an_identity_only_one_compiler_can_compute_probe_test`：`g4_the_standard_root_does_not_move_and_holds_no_code`
*   `an_obstruction_the_gate_did_not_read_probe_test`：`g4_an_effect_does_not_move_an_address`
*   `an_overwrite_that_was_a_coin_flip_probe_test`：`g1_identity_is_a_red_line`
*   `atomic_write_probe_test`：`p2_format_moves_only_when_declared`
*   `depth_belongs_to_the_savepoint_probe_test`：`g1_the_standard_root_digest_does_not_move`、`g5_an_all_solid_program_commits_unchanged`、`r3_top_level_pure_computation_is_not_solidified_either`
*   `every_byte_or_none_probe_test`：`p1_the_root_caid_does_not_move`、`r5_the_store_format_says_it_changed`
*   `evidence_that_needs_a_binary_probe_test`：`e5_this_engine_still_recomputes_cef5e484_for_the_artifact`
*   `genesis_test`：`seed_caids_are_stable`
*   `knob_that_does_nothing_probe_test`：`p1_plain_commit_root_is_unchanged`
*   `limit_you_cannot_choose_probe_test`：`p1_plain_commit_root_is_unchanged`
*   `name_points_at_remedy_probe_test`：`p2_fuel_blur_caid_holds`
*   `node_identity_probe_test`：`pin_ordinary_value_caids_do_not_move`
*   `nothing_here_or_nothing_you_can_see_probe_test`：`g1_a_fresh_workspace_lands_both_declarations`
*   `one_tag_eight_meanings_probe_test`：`g3_the_standard_root_does_not_move`、`g4_the_values_this_arc_does_not_touch_keep_their_addresses`
*   `oodp_packet_format_probe_test`：`pin_ordinary_value_caids_do_not_move`
*   `print_what_can_be_read_probe_test`：`p4_root_caid_does_not_move`
*   `privileged_effect_audit_probe_test`：`pin_ordinary_value_caids_do_not_move`
*   `snapshot_not_a_reading_probe_test`：`p3_a_universe_without_a_blur_keeps_its_root`、`p4_a_unify_side_blur_has_a_literal_address`、`p5_a_morphism_bearing_universe_has_its_new_root`
*   `the_address_names_the_point_probe_test`：`g4_identity_is_a_red_line`
*   `the_last_thing_it_says_probe_test`：`g3_identity_is_a_red_line`
*   `two_values_one_address_probe_test`：`g3_the_standard_root_does_not_move`
*   `universe_determinism_probe_test`：`pin_ordinary_value_caids_do_not_move`

---

## 8. 交付回報（交付方填；本行以上一字不得動）

### 8.1 射程逐項對照

### 8.2 順手改動（逐項指名）

### 8.3 工單哪裡是錯的

### 8.4 工單指名要你回答的問題

### 8.5 探針

### 8.6 數字

### 8.7 你認為需要改規格之處

## 9. 驗收（驗收方填）
