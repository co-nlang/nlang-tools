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

1. **I1。** Combo 每個條目在鍵前面寫一個軸位元組。優先序沿 REAL_03 §6.2：系統 1、元 2、型別 3、邏輯 4、資料 5、私有 6。7（`~@`）與 8（`~/`）引擎不產生。排序是軸位元組，然後鍵的 UTF-8。`legacy_fields` 仍是 5，`legacy_local` 仍是 6。
2. **I1。** Union 是 `0x05`、LEB128 個數、成員依自己的 V 位元組排序並去重。凍結路徑仍是成員串接、排序、去重，沒有標記、沒有個數。
3. **I1，找到並留下。** `Value::Bottom` 只寫 `0xFE`，原因、路徑、訊息不進位元組，而 `PartialEq` 比較 `BottomDetail`。工單說其餘 V 與 v0.84.0 相同，r6 釘的就是那張表，所以這一處沒有改編碼。原子的 effect／rank、`hash_recursive_with_salt`、`Commit::content_hash` 都維持原樣。Expr／`EX_COMBO` 沒有軸位元組；軸只在值的 Combo 上。
4. **I2。** 跨軸 `--pin` 落地（r3）。
5. **I3。** 紀元一的倉照開，答案與 v0.84.0 相同；寫過一次之後舊物件還在；凍結編碼對得上的竄改仍讀得出（Q3）；gc 不刪可達物件。`encoding4_repo`、`layout2_framed_repo`、`layout5*`、`layout6_signed_repo`、`layout7_repo`、`pre_sentinel_repo` 維持綠。
6. **I4。** 新摘要是 SHA-256(`0x80 ‖ V`)。巢狀值不加 `0x80`。凍結路徑是 v0.84.0 的 `serialize_bn`／`content_digest`，e1 的十個摘要對得上。`0x80` 不是紀元一任何合法值的首位元組，所以兩套位址空間不相交。
7. **I5。** `encoding=5` 的倉在寫下第一個新物件之前，把 `objects.format` 改成 `encoding=6`。新倉宣告 `encoding=6`。唯讀不改宣告。路徑已存在的重寫不抬。`encoding=4` 的普通寫入不抬；`migrate` 把已宣告 ≥4 的編碼抬到 6，更舊的編碼留在原處。記憶體裡的 `encoding` 欄位在檔案抬到 6 之後仍是 5；編解碼對 5 與 6 走同一支。
8. **I6。** §2 的新紅線由 r6 釘住。附錄裡的摘要字面，以及本引擎寫出或 migrate 之後的 `encoding=5` 字面，改成這支引擎算出的值。

### 8.2 順手改動（逐項指名）

1. `bn_serial.rs`：軸位元組、Union 的 `0x05` 與個數、外層 `0x80`、`with_legacy_identity`（執行緒局部深度，含 unwind）。
2. `value.rs`：`cache_id` 只在當前紀元讀寫。
3. `storage.rs`：`OBJECT_ENCODING_VERSION = 6`；讀取先當前再凍結；`encoding=5` 的倉在第一個新物件之前改宣告。
4. `store_codec.rs`：提交本文的位址走值的 `content_hash`。
5. `gc.rs`：未加框的值走雙紀元。未加框的提交仍用 `Commit::content_hash`。
6. `oodp.rs`：客戶端兩處改走 `value_matches_stored_address`。伺服器的 fetch 仍是 `get_value`。
7. `lib.rs`：`StandardRootSet::from_roots` 先收當前摘要，再在凍結旗標下收同一組形狀。
8. `genesis.rs`：`SEED_*`。`seed_caids_are_stable` 比的就是這些常數；`all_seeds` 沒有別的呼叫點。註 `Q-083／D106 紀元`。
9. 附錄測試：只改摘要字面，以及本引擎寫出或 migrate 之後預期的 `encoding=5` → `encoding=6`。行尾註 `Q-083／D106 紀元`。`a_refusal` r11 的輸入表仍是 `encoding=5`。
10. `a_store_written_in_another_language_probe_test` 的 g6 讀的是 `encoding=4` 夾具上的舊物件。新鮮倉的 `ROOT_CAID`／`STANDARD_ROOT` 改到新摘要；夾具名留在 `FIXTURE_ROOT_CAID`（`932a9f9d…`）與 `FIXTURE_STANDARD_ROOT`（`7038e250…`）。
11. `an_identity_only_one_compiler_can_compute_probe_test` g4 的標準根路徑從 `sha256/70/38e250…` 改到 `sha256/c9/6c2937…`。
12. `evidence_that_needs_a_binary_probe_test` e5：重算摘要由 `cef5e484` 改為 `7c147a6beb9ee793dcfb2cbbb411c16cfcda89f551376120f4d294a1f97853e8`。
13. `a_commit_that_is_a_value_probe_test` g5 的對照 `{ a: 1 b: "x" }`（沒有逗號）是 `a379a89dcb55a6653a1f867883e52e72f059829745df7b83fabbe104c213c65d`。
14. 未改：三支新探針、`epoch1_repo`、`what_the_top_dropped_probe_test` 的 `STD_HEX`、`main.rs`、`pin_probe_test`、版本（仍 `0.84.0`）、`Cargo.lock`、`spec/`、`meta/`、`conformance/`、`CHANGELOG`、`REAL_03`。linux 上 lib 的 14 條舊警告、windows 上的 15 條，都沒有 cargo fix。

### 8.3 工單哪裡是錯的

無。

### 8.4 工單指名要你回答的問題

**Q1**

倉的位址：

1. 寫入（`put_value`、`put_root`、`put_commit`、`commit_body_address`）只用當前紀元。
2. 讀取驗證（`get_value`、`get_root`、加框 `get_commit` 的 V2、`open_commit` 在 v1 對不上時的回退）先算當前摘要，不中再在 `with_legacy_identity` 下重算。對不上的錯誤帶的是當前摘要。
3. gc 未加框的值用同一套雙紀元。未加框的提交與 `Commit::content_hash` 是父提交加根的另一套演算法，不包 `0x80`。
4. oodp 伺服器的 fetch 走 `get_value`。客戶端兩處走 `value_matches_stored_address`。裸的 64 hex 是 `#unparseable_caid`。
5. 標準根架對每個已出貨形狀收當前摘要，再收凍結摘要。
6. `cache_id` 只在旗標關閉時讀寫。凍結重算不讀、不寫它。
7. 載入的紀元一值，在旗標外面的 `content_hash()` 是新位址。HEAD 與父提交仍保存舊位址，當作指標。把紀元一的宇宙再提交，會寫一個新根物件，舊物件留在原位（g2）。那是新的身分。

記憶體裡的身分：合一早退、`pattern_branch_name`、`disc.identify`、force／cycle memo、活值的 lattice sketch，都走當前編碼。凍結旗標只在核對紀元一已經寫下的摘要，以及索引歷史標準根時打開。lattice 的欄位振幅雜湊 V，不加 `0x80`。`{ hello: "world" }` 的 sketch 前綴因此沒動，摘要動了。

**Q2** 對照引擎是 `/home/gali/nlang-baselines/v0.84.0-verify/target/release/oo`。本引擎是這次釋出的 `oo`。

（a）`epoch1_repo` 的複本，本引擎只讀過。`layout=9`，`encoding=5`。本引擎與 v0.84.0 的 `status` 都是 rc 0：

```
Standard root dependency: 7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911 (available)
Universe is static (no staged changes).
```

`log` rc 0，兩筆：HEAD `f8e94c4719238893d5662b01365f8c3dd0300c2e449ea01504f2b95ca495e7fd`，`epoch one, second`，`2026-10-12T13:32:34.889Z`；第一筆 `2858af651809a9aa86cddd06558846d326fbb15a196d874a21ca7559c06f93a3`，`epoch one`，`2026-10-12T13:32:34.685Z`。`eval _.t` rc 0，印 `{ int: 1 }`。

（b）同一夾具，本引擎 `evolve`＋`commit` `y: 1`，rc 0。之後 `layout=9`，`encoding=6`。v0.84.0 的 `status`、`log`、`eval _.t` 都是 rc 1，stderr：

```
Error: object encoding version 6 is not supported by this engine (understands encoding 1 through 5); refusing to open
```

（c）新鮮的 `x: 0` 提交，同樣 `layout=9`／`encoding=6`。v0.84.0 的 `status` 與 `log` 都是 rc 1，同一句。

**Q3** 紀元一複本，竄改前 `eval _.t` 是 `{ int: 1 }`，rc 0，`encoding=5`。根物件 `99c28198decc1052fcc65212ab15aac6d9a30586546835ac660a38bc744ae47c` 裡的第一處 `int: 1` 改成 `@int: 1` 之後，`eval _.t` 是 `{ @int: 1 }`，rc 0，宣告仍是 `encoding=5`。沒有 `#caid_mismatch`。凍結編碼仍對得上，這就是留下的那個弱點。

對等點是這份原始碼上 `Ouroboros::init` 加 `oodp::serve_request` 量的（釋出閘之前；之後這條路徑沒有再改）。裸 64 hex 是 `#unparseable_caid`。`hash:sha256:v1:99c28198…` 與倉裡那條 v2 位址，竄改前後都是 `OODP Served`、成功。竄改後 `get_value` 的欄位 `t` 印 `{ @int: 1 }`。線上的 JSON 本文沒有拿來對 `int: 1` 這幾個字。

**Q4** 釋出 `oo` 上量的。沒有一個留在舊位址。

| 值 | 新位址 |
| :-- | :-- |
| `x: 0` | `b74d775816f6b5be6cc152577b7556f6c8fdcc63433b0e31068cc2484a94ae0f` |
| `v: 1 + 1` | `df91a8ad239ea8727c3a03a71eb4b0a787312b92306db4d8b124a73faaf40199` |
| `f: (x -> x + 1)` | `258a1a08adfd0340cdbe103e4f4851bea107a67e298e72d6679badc0595bfd1f` |
| `k: { a: 1, _: 9 }` | `420a0c9f4592b49ace8c8aacec690fd0a70953b1bd2158260e68c3acce710d9e` |
| `m: (_ -> 1)` | `cf11d2ff2e95c11ecbae5ceba00ece7c60f87c7d81149287ffebce1b551a89a6` |
| 標準根 | `c96c29378c9395936ea4999db37e9b6a28291b57a47d4c016ca3cdf6833fed3b` |
| `0` | `3865e1f4065532383839cf2bb9a68cc810c8d2eb25a80d14d376cf07e3865f49` |
| `"a"` | `ba1a6dd03186fd2e05d1b1c27b13ab52ca1deabbf91f59dbde87eb093267e7ed` |
| `{}` | `0a635ac8c2ad27f595e9687005e35896de952f3df549b892e66fabaf1c724c3e` |
| `{ a: 1 }` | `cd254f289505a7e84c4e0bb780f27f3e9028c0b4489a8e0f1a1342a7af755d1d` |
| `f: (x -> x)` | `2a19b65733d0e1c57cc421df82645413a3a7fb6e6bfb341d8a0e21d7d96fae25` |
| `#blur`（`identify_and_store`） | `96520487af465c8591d4778485116f9db5489f14e2fc40de49bd9251a61e2602` |

`app: { k1: 1 }` 的根是 `394921b382efcd56b72b532131934ba1ba046735ddaf716baa1e0ca838a0459d`（舊 `932a9f9d…`）。`name_points` 那個 fuel 耗盡的 `#blur` 是 `4069c7cb91f8ae8a84b955127a7e599d99e7d5f696467077b808c1ed84b7143d`，與上面這個字面 `#blur` 不同。

**Q5** `migrate --grant migrate` 不搬位址。`main.rs` 的鎖出句子沒有改。

`encoding4_repo` 複本：之前 `layout=2`／`encoding=4`，HEAD `hash:sha256:v1:3ced204289895c8656b7d33ffb0a414d604841a865c411a26205ea3e082d36d3`，物件 3 個。rc 0。之後 `layout=9`／`encoding=6`，HEAD 與物件路徑不變。印出：

```
Migrating this store from layout=2 to layout=9 will make it unopenable by oo v0.26.0 through v0.74.0. That includes every engine that opens layout=5 (oo v0.44.0 through v0.74.0); none of them open layout=9. Object encoding advances from encoding=4 to encoding=6.
Migrated store layout to layout=9 and object encoding to encoding=6.
```

這句停在 v0.74.0。開得了 `layout=9`、開不了 `encoding=6` 的引擎（v0.75.0 到 v0.84.0）沒有被點名。

`epoch1_repo` 複本：之前 `layout=9`／`encoding=5`，HEAD 是上面的 `f8e94c47…`，物件 5 個。rc 0。之後 `layout=9`／`encoding=6`，HEAD 與物件路徑不變。印出：

```
Advancing object encoding from encoding=5 to encoding=6 locks out no engine.
Migrated object encoding to encoding=6.
```

這句與 Q2（b）不合：v0.84.0 拒絕 `encoding=6`。

**Q6** `shipped_standard_roots` 依序送四個形狀：`root_with_system()`、`as_shipped(quoted_names)`、`as_shipped(v0_22)`、未投影的 `v0_22`。`from_roots` 先插入這四個的當前摘要，再在凍結旗標下插入同一順序。架上八個：

1. 現行形狀，紀元二 `c96c29378c9395936ea4999db37e9b6a28291b57a47d4c016ca3cdf6833fed3b`
2. 引號名，紀元二 `a201cea32fc8e7a159cfc822c58c06edfec3d031d1703ef1cc9b4fb7890e9246`
3. v0.26 `as_shipped(v0_22)`，紀元二 `182d17736188643bffd0cde7176f76f1be3a990e35463a3c4c97f3864c538032`
4. pre-cas `v0_22_standard_root()`，紀元二 `372ffae835141a3317f5b9282456f7abddb836459a6f416b4c4bd7a9b5a7a1a9`
5. 現行形狀，紀元一 `7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911`
6. 引號名，紀元一 `229be911057aaff665b13691115e5ee271d2007b649ebbfaef40cf70b6802c24`
7. v0.26，紀元一 `2da5b71371649291cfa5dc5d0cd019464d248e98645b3901938e1c08d2172c2c`
8. pre-cas，紀元一 `65f52e2da48baa550d7340c0fdc214fd1f9925577a96ffec59bc34f8b2bcbe72`

1 與 5 在釋出 `oo` 的 `status` 上又對過。2–4 與 6–8 是同一份原始碼的標準根架走出來的；6–8 的舊測試仍綠。

### 8.5 探針

探針與夾具未改。釋出閘裡：CLI 探針 11／11（5.31s），union 2／2，epochs 4／4（0.34s），`pin_probe_test` 15／15（2.26s）。

### 8.6 數字

1. 版本仍是 `oo` 0.84.0。新倉是 `layout=9`／`encoding=6`。
2. 釋出全樹三輪，`--jobs 1`，`--test-threads=1`。每輪 `test result:` 274 行，`Running` 271，passed 2643，failed 0，`^error` 0，exit 0。去掉 `finished in …s` 之後，以 `test result:`、`test `、`Running `、`running ` 開頭的行三輪相同。
3. conformance 162／162，exit 0。
4. 新鮮目錄裡 `~%Math./add (1, 2)` 是 3，`(1, 3)` 是 4，rc 0，該目錄沒有 `.oo`。euid 1000。`oo-ephemeral-*` 為 0。
5. `cargo check --release --offline -p oo -p nlang-interpreter -p nlang-parser --target x86_64-pc-windows-gnu` exit 0，`^error` 0。該 target 上 lib 15 條警告。linux 上 lib 14 條。都沒有修。
6. 未 push。

### 8.7 你認為需要改規格之處

`REAL_03` 的改寫歸驗收方。Q5 那兩句鎖出文字是引擎印的，留在引擎裡。開放範圍的寫法未動。

## 9. 驗收（驗收方填）

### 9.1 第一輪驗收：**不受理，開 R-1（三項；第一項是驗收方的錯）**

**照收的部分**〔量，交付 `2b6a29e`〕：差異純度——三支探針、夾具、`pin_probe_test`、Cargo 檔、本檔 §8 以上一字未動；身分紅線與參考編碼逐根相同（`x: 0` `b74d7758…`、`v: 1 + 1`／`1+1` `df91a8ad…`、`f` `258a1a08…`、`k` `420a0c9f…`、`m` `cf11d2ff…`、標準根 `c96c2937…`；新倉 `layout=9`／`encoding=6`；`add` 3／4 不建 `.oo/`）；全樹一輪 2643 passed／0 failed、rc=0；交叉編譯 0 error；conformance 162／162。8.1 #3 留下 `Value::Bottom` 只寫 `0xFE`——對，成因是纖維不入址（D65）。Q3 的殘留弱點照工單收下。

**F1（驗收方的錯，不是交付的錯）：工單 §2 第 3 點、I5、r5 違反 `REAL_02` §5.1.1 三款 Core Requirement。**「寫入不得改動它所找到的宣告（MUST NOT）」「宣告的推進必須被明示要求，且動手前說出代價（MUST）」「宣告未推進者不得收到它宣告不了的東西——寫出的位元組必須是該宣告所對應的形式（MUST NOT）」。〔量，交付〕`epoch1_repo` 的複本上一次普通的 `commit`，畫面上沒有代價、沒有人要求遷移，`objects.format` 由 `5` 變 `6`，v0.84.0 自此 `refusing to open`——§5.1.1 的 v0.33.0 判例逐字是這個形狀（「一次沒有人要求的遷移，替使用者做了一個不可逆的決定」）。交付照工單做對了；錯在工單把 O63 的「格式閘」寫成了「寫入時自己升閘」。**更正後的規則**：

*   宣告 `encoding≤5` 的倉，**所有寫入**（值、根、標準根、提交）以**凍結編碼**定址，**寫進值裡的名字**（`%rules` 的分支名、`%bodies` 的鍵——見 F2）也以凍結編碼算：寫下的位元組必須是 v0.84.0 會寫的（r10）。
*   凍結編碼**分不開**的寫入——該紀元一位址已經放著另一個 `=` 分得開的值——在**寫下任何東西之前**具名拒絕，並指名 `oo migrate --grant migrate`（r7）。這就是那個 `--pin` 的靜默空操作在未遷移的倉裡的歸宿：變成一句具名拒絕。
*   宣告只由 `migrate` 推進（g6）；本引擎新建的倉仍是 `encoding=6`（r5，已收窄）。宣告 `encoding=6` 的倉：寫一律新編碼，讀「先新後舊」（遷移前的物件仍在）。
*   D106 乙那句「新寫入一律用新編碼」更正為「**宣告 `encoding=6` 的倉裡**新寫入一律用新編碼」。

**F2（交付缺陷）：從紀元一倉讀進來的表，與本引擎現在寫下的同一張表不是同一張表。** 頂層的表求成值後以名字存分支——`%rules` 的鍵是 `{content_digest(模式)}`、`%bodies` 的鍵是 `{content_digest(本體)}`——**那是寫進值裡的紀元一摘要**。§5 Q1 問的正是「有沒有哪一處把讀進來的舊物件的舊位址又當成它現在的身分用」，8.4 答沒有。〔量，夾具 `epoch1_tables_repo`（真 v0.84.0 所建，本次新增）〕

| | 交付 | v0.84.0（同一個倉） |
| :-- | :-- | :-- |
| `_.k = { @{ @str }: 9, @{ @int }: 1 }` | `#false` | `#true` |
| `(_.k & { @{ @str }: 10 }) "s"` | `9 \| 10` | ⊥ `#conflict` |
| `(_.b & { @{ "k" }: { c: 3 } }) "k"` | `{ a: 1 b: 2 } \| { c: 3 }` | `{ a: 1 b: 2 c: 3 }` |
| `(_.b & { @{ "k" }: { a: 5 } }) "k"` | `{ a: 1 b: 2 } \| { a: 5 }` | ⊥ `#conflict` |

一個約束兩個分支（D101），三個靜默錯答案。**不變式**：讀進來的值與本引擎此刻寫下的同一個值，在 `=`、合併、施用之下無從分辨（g7）；反方向同理——寫進紀元一倉的名字是紀元一的（r10）。**凡是寫進值裡、由位址導出的名字都在射程內**；`%rules` 與 `%bodies` 是驗收方找到的兩處，你找到別的一併處理並在回報裡列出。
〔驗收方自陳〕I3 的探針漏了這一格：`epoch1_repo` 只放了**一層下**的表——那是存成程式碼的 thunk，不帶名字。

**F3（交付自己在 Q5 量到的）：`migrate` 的代價句說錯了它鎖掉誰。** F1 之後 `migrate` 是通往 `encoding=6` 的**唯一**入口，它的代價句是使用者唯一的警告。〔量，交付〕`layout=9`／`encoding=5` 的倉 ⟹「locks out no engine」——v0.84.0 實際被鎖掉；`layout=2`／`encoding=4` ⟹ 上界停在 v0.74.0，漏了 v0.75.0–v0.84.0。**每一句都要是真的**：上界在推進到 `encoding=6` 時是 v0.84.0；「every engine that opens layout=5 … none of them open layout=9」那一子句對 v0.75.0–v0.84.0 不成立（它們開得了 `layout=9`，開不了 `encoding=6`），要改寫成真話（r9）。下列四支既有測試釘的是舊上界 v0.74.0，**宣稱不變（代價句點名鎖掉的引擎），上界隨真話改**：`a_refusal_written_as_an_answer_probe_test` `r11_the_cost_names_the_oldest_engine_it_locks_out`、`a_savepoint_that_knows_where_it_stood_probe_test` `r6_a_migrated_layout7_store_records_points`、`a_commit_that_is_a_value_probe_test` `r6_a_migrated_store_writes_commits_that_are_values`、`a_signature_that_signs_the_commit_probe_test` `r8_a_layout6_store_is_migrated_on_request`。

**探針的變動（驗收方，本輪）**：CLI 探針 r5 收窄為「本引擎新建的倉宣告 `encoding=6`」；新增 g6（普通寫入不動宣告）、g7（紀元一的表照舊答——v0.84.0 上綠，故為守衛；初寫為 r8）、r7（紀元一分不開的寫入具名拒絕，`migrate` 之後落地）、r9（代價句點名 v0.84.0）；新檔 `crates/oo/tests/an_axis_the_address_forgot_bytes_probe_test.rs` r10（寫入後每個物件仍在它的紀元一位址上，且原封未動的表的名字與 v0.84.0 寫的相同；**對照：夾具本身先過同一檢查**）——獨立成檔，因為它用凍結編碼讀物件，在 v0.84.0 上編譯不過，而 CLI 探針必須在 v0.84.0 上編譯，守衛才校準得了；新夾具 `epoch1_tables_repo`。
〔校準〕v0.84.0：CLI **7 綠 8 紅**（g1–g7 綠；r1–r7、r9 紅，皆在一般斷言）；bytes 檔編譯失敗（API 不存在）。交付 `2b6a29e`：CLI **11／15**（g6、g7、r7、r9 紅）、bytes **0／1**（紅在「after a commit」，對照組綠）。R-1 參考實作（`q083-ref` `beb41e3`）：CLI **15／15**、bytes **1／1**、`pin_probe` 15／15、epochs 4／4、union 2／2；全樹除〔附錄〕的 56 支身分字面值外只多上列 F3 四支，**舊倉測試全綠**。**變異**：讀入不換名 ⟹ g7；寫入不換名 ⟹ r10；不拒撞址 ⟹ r7；寫入升宣告 ⟹ g6。

**R-1 回報**：寫在下面 9.2（交付方填），格式同 §8；三輪全樹、交叉、conformance、身分紅線重報；**Q2、Q5 依 F1／F3 重量重答**；並答 **Q7**：(a) 你找到幾處「寫進值裡、由位址導出的名字」，各在哪；(b) 宣告 `encoding=5` 的倉被本引擎寫過之後，用 v0.84.0 開它——`log`、`eval _.k "s"`、`(_.k & { @{ @str }: 10 }) "s"` 各得什麼（`epoch1_tables_repo` 的複本上，本引擎先提交 `y: 1`）。

### 9.2 R-1 交付回報（交付方填）
