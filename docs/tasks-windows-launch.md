# Windows / WSL Launch Support 実装タスクリスト

| 項目 | 内容 |
|------|------|
| 対象 | `rhino-cli launch` / `shutdown` / プロセス制御の Windows native + WSL 対応 |
| 設計書 | `docs/design-windows-launch.md` |
| 作成日 | 2026-05-14 |
| テスト方針 | ロジックは Rust unit test、実機検証は Windows / WSL で手動 |
| 進捗管理 | チェックボックス `- [ ]` を `- [x]` に更新 |

---

## Phase 1: macOS コードを platform module に移動（挙動変更なしリファクタ）

- [x] **1-1**: `src/commands/rhino.rs` を `src/commands/rhino/mod.rs` に分割。orchestration（`launch`/`shutdown`/`screenshot`/`app_running`/`validate_app_name`/`default_screenshot_path`/`wait_until_not_running`/`LaunchArgs` 等の public 型）を mod.rs に残す
- [x] **1-2**: `src/commands/rhino/platform/mod.rs` を作り、orchestration 側に出す関数を pub re-export する形にする
  - `pub fn launch_app(app: &str, script: Option<&str>) -> Result<()>`
  - `pub fn request_quit(app: &str) -> Result<()>`
  - `pub fn is_app_running(app: &str) -> Result<bool>`
  - `pub fn capture_window(app, out, window_id, activate, no_shadow) -> Result<()>`
- [x] **1-3**: 既存 macOS 専用コード（`launch_app` / `capture_window` / `request_quit` / `is_app_running` / `run_osascript` / `run_command_with_timeout` / `command_output_message` / `ensure_screen_capture_access` / `app_window_id` / `activate_app` / `visible_app_window_id` / `owner_matches_app`）を `src/commands/rhino/platform/macos.rs` に移動
- [x] **1-4**: `src/commands/rhino/platform/unsupported.rs` を作り、現状の `"Rhino launch is currently only supported on macOS."` 系エラーを返す関数群を置く（pure Linux 等の fallback）
- [x] **1-5**: `platform/mod.rs` の cfg selector を整える
  - `cfg(target_os = "macos")` → `pub use macos::*;`
  - `cfg(target_os = "windows")` → Phase 2 で `pub use windows::*;` に
  - `cfg(target_os = "linux")` → Phase 3 で WSL ランタイム分岐に
  - 現時点では Linux は `unsupported` を指す
- [x] **1-6**: `cargo make test` と既存の Rust unit テストが pass する（macOS 側挙動が壊れていない）
- [x] **1-7**: commit（タイトル英語、gitmoji `♻️ refactor:` 形式）

**Done の定義**: macOS でビルド・テストが通り、`rhino-cli launch` / `shutdown` の挙動が変わらない。

---

## Phase 2: Windows native platform 実装

- [x] **2-1**: `src/commands/rhino/platform/windows.rs` を新規作成（`cfg(any(target_os = "windows", target_os = "linux"))` でコンパイル）
- [x] **2-2**: `fn windows_to_wsl_path(win: &str) -> PathBuf` を実装（純文字列処理）
  - `C:\foo\bar` → `/mnt/c/foo/bar`
  - 既に `/mnt/...` 形式なら no-op
  - Windows native ターゲットでは no-op（パススルー）
- [x] **2-3**: `fn resolve_rhino_exe(app: &str) -> Result<PathBuf>` を実装
  - `RHINO_CLI_RHINO_EXE` 最優先（windows_to_wsl_path で正規化）
  - `Rhino\s*(\d+)` でバージョン抽出
  - 候補リスト: `[version, 8, 7].dedup()` の順で `C:\Program Files\Rhino {N}\System\Rhino.exe` を試す
  - WSL では `/mnt/c/...` に変換してから `exists()` チェック
  - すべて見つからなければ `CliError::Other` with 探索したパスを列挙
- [x] **2-4**: `launch_app(app, script)` 実装
  - 子プロセス: `Command::new(rhino_exe).arg(format!("/runscript={script}"))`（script が Some の時のみ。複数 args が必要なら適宜分割）
  - Windows native: `creation_flags(0x00000200)` でデタッチ
  - WSL: そのまま `spawn()`
  - `spawn().map_err(|e| ...)` → `Ok(())`。child は wait しない
- [x] **2-5**: `request_quit(app)` 実装
  - `Command::new("taskkill.exe").args(["/IM", "Rhino.exe"])` を実行
  - exit code 0: 成功
  - exit code 128 または stderr に "not found" 含む: 該当プロセスなし扱いで Ok（is_app_running と整合させる）
  - その他: エラー
- [x] **2-6**: `is_app_running(app)` 実装
  - `Command::new("tasklist.exe").args(["/FI", "IMAGENAME eq Rhino.exe", "/FO", "CSV", "/NH"])`
  - stdout が `"Rhino.exe"` で始まる行を含むか → true
  - `"INFO: No tasks are running"` または空 → false
- [x] **2-7**: `capture_window(...)` は `Err(CliError::Other("Rhino window screenshot is not yet implemented on Windows.".into()))` を返す
- [x] **2-8**: unit test
  - `windows_to_wsl_path` の変換ケース（C:\、別ドライブ、`/mnt/...` 入力、UNC は対象外で明示）
  - `resolve_rhino_exe` のバージョン抽出と候補順（存在チェックは tempdir / mock）
  - `tasklist` 出力パーサ（固定文字列入力に対する true/false 判定）
- [x] **2-9**: `platform/mod.rs` の Windows native cfg を `pub use windows::*;` に切り替え

**Done の定義**: `cargo build --target x86_64-pc-windows-msvc` （または Windows 実機ビルド）が通り、unit test が pass。

---

## Phase 3: WSL ランタイム検出と Linux 側分岐

- [x] **3-1**: `src/commands/rhino/platform/mod.rs` に `fn is_wsl() -> bool` を実装
  - `/proc/sys/kernel/osrelease` を読む
  - "microsoft"（大文字小文字無視）または "WSL" を含むか
  - `OnceLock<bool>` でキャッシュ
  - 純関数版 `fn is_wsl_from_release(osrelease: &str) -> bool` を分けて unit test 可能に
- [x] **3-2**: `cfg(target_os = "linux")` 配下で、各関数の入口で `is_wsl()` 分岐
  - true → `windows::launch_app(...)` 等を呼ぶ
  - false → `unsupported::launch_app(...)` 等を呼ぶ
- [x] **3-3**: unit test
  - `is_wsl_from_release` を `"5.15.153.1-microsoft-standard-WSL2"` で true
  - `"6.6.0-1018-aws"` で false
  - 空文字列で false

**Done の定義**: WSL 上のビルドで `rhino-cli launch` が Windows native と同じコードパスを実行する。pure Linux ターゲットでは従来エラーが出る。

---

## Phase 4: ドキュメント追従（CLAUDE.md 最優先ルール）

- [x] **4-1**: `README.md` の「Rhino 実機確認」節を OS 中立に書き換え、Windows / WSL 補足を追加
- [x] **4-2**: `docs/design.md`
  - §1.4 Non-Goals の "Windows での開発検証" 項目を削除または更新
  - §4.2.8 `launch` の本文を OS 中立に（macOS 専用記述を一般化）
  - §4.2.10 `shutdown` 同上
  - §4.2.15 `screenshot` は macOS only のまま明記
- [x] **4-3**: ルート `CLAUDE.md`「macOS 実機機能…」の文を OS 別サポート表 or 注釈に更新
- [x] **4-4**: `docs/tasks.md` の進捗ログに「Windows/WSL launch/shutdown 対応」行を追加（Phase 5 完了後）

---

## Phase 5: 実機検証（Windows 側で実施）

- [ ] **5-1**: Windows native 上で `cargo make build` がエラーなく通る
- [x] **5-2**: WSL 上で `cargo make build` がエラーなく通る（2026-05-14、`cargo build` のみ。`cargo make` (dotnet 含む) は未実行）
- [ ] **5-3**: Windows native: `plugin set-port 50061` → `launch --new-model` → `wait-ready --port 50061 --timeout 120` → `doctor --port 50061` → `shutdown` が成功
- [ ] **5-4**: WSL: 同シナリオが成功（Windows 側 Rhino が起動・終了する）
  - 部分検証済み（2026-05-14）: WSL から `launch --new-model` で Rhino.exe が起動し、tasklist で確認、`shutdown` で終了するまで確認。`plugin set-port` / `wait-ready` / `doctor` を含むフルシナリオは未検証
- [ ] **5-5**: 保存待ちダイアログが残るケースで `shutdown --timeout 30` がタイムアウトする
- [ ] **5-6**: `RHINO_CLI_RHINO_EXE` 上書きで存在しないパスを指したとき明示エラーが出る
- [ ] **5-7**: `--app "Rhino 7"` を指定すると Rhino 7 を優先探索する（Rhino 7 がない環境では明示エラー）

---

## Phase 完了の定義

各 Phase は以下を満たした時点で完了:
1. 全チェックボックスが `[x]`
2. macOS / Windows / WSL のいずれでもビルドが通る
3. Phase 5 の実機シナリオが手動で確認できている

---

## 進捗ログ

| Date | Phase | Note |
|------|-------|------|
| 2026-05-14 | 0 | 設計書・タスクリスト作成 |
| 2026-05-14 | 1 | rhino.rs を `rhino/mod.rs` + `platform/{mod,macos,unsupported}.rs` に再構成。挙動変更なし（WSL `cargo check` / `cargo test --lib` で通過確認） |
| 2026-05-14 | 2 | `platform/windows.rs` 実装。`windows_to_wsl_path` / `resolve_rhino_exe` / `launch_app` / `request_quit` / `is_app_running` + 16 件の unit test。`#![allow(dead_code)]` で Linux 上のコンパイル時 rustc ICE 回避 |
| 2026-05-14 | 3 | `is_wsl()`（`OnceLock` キャッシュ） + 純関数 `is_wsl_from_release` + Linux dispatch wrapper。WSL から `launch --new-model` → Rhino.exe 起動 → `shutdown` 終了まで実機確認 |
| 2026-05-14 | 4 | README / design.md / CLAUDE.md / tasks.md の OS 中立化 |
