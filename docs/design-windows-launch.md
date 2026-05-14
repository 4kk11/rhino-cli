# Windows / WSL Launch Support 設計書

| 項目 | 内容 |
|------|------|
| 対象 | `rhino-cli launch` / `shutdown` / 関連プロセス制御の Windows native + WSL 対応 |
| 設計日 | 2026-05-14 |
| ステータス | Draft |
| 関連 | `docs/design.md` §4.2.8 (`launch`) / §4.2.10 (`shutdown`) |
| 実装タスク | `docs/tasks-windows-launch.md` |

---

## 1. 目的

WSL から `rhino-cli launch` / `shutdown` で Windows 側の Rhino を起動・終了できるようにする。同じ実装で Windows native ビルドでも動かす。`screenshot` は本タスクのスコープ外（PrintWindow 実装が重いため別タスク）。

## 2. 現状

`src/commands/rhino.rs` の `launch_app` / `request_quit` / `is_app_running` / `capture_window` は `#[cfg(target_os = "macos")]` のみ実装。Linux ターゲット (= WSL バイナリ) は `"Rhino launch is currently only supported on macOS."` を返して失敗する。

## 3. 設計方針

### 3.1 WSL と Windows native を 1 つの platform 実装でまとめる

WSL は `.exe` を透過実行できる。Windows 用プリミティブ（Rhino.exe spawn、`tasklist.exe`、`taskkill.exe`）を 1 本書けば、ランタイムの WSL 判定でそのまま WSL からも使える。

- Pure Linux（非 WSL）は引き続き未サポート。明示エラーで返す
- macOS 既存実装には一切手を入れない（モジュール移動のみ）

### 3.2 macOS のセマンティクスに揃える

| 操作 | macOS 既存 | Windows / WSL 新規 |
|------|-----------|-------------------|
| 起動 | `open -a "Rhino 8" --args -runscript <SCRIPT>` | `Rhino.exe /runscript=<SCRIPT>` をデタッチ spawn |
| 終了依頼 | `osascript -e 'quit app "Rhino 8"'`（保存ダイアログ尊重） | `taskkill.exe /IM Rhino.exe`（`/F` なし → WM_CLOSE 送信、保存ダイアログ尊重） |
| 起動判定 | `osascript -e 'application "..." is running'` | `tasklist.exe /FI "IMAGENAME eq Rhino.exe" /FO CSV /NH` をパース |

いずれも OS レベルの shell-out。`sysinfo` 等のクレートは導入しない（macOS 側のビルド依存に影響させないため）。

## 4. モジュール分割

```
src/commands/rhino/
├── mod.rs              # launch / shutdown / screenshot の orchestration（既存ロジック）
└── platform/
    ├── mod.rs          # cfg + WSL ランタイム判定で実装を選択
    ├── macos.rs        # 既存 macOS コードを移動
    ├── windows.rs      # Windows native + WSL 共用
    └── unsupported.rs  # macOS / Windows / WSL 以外（pure Linux など）
```

`platform/mod.rs` が orchestration 側に出す関数:

- `launch_app(app: &str, script: Option<&str>) -> Result<()>`
- `request_quit(app: &str) -> Result<()>`
- `is_app_running(app: &str) -> Result<bool>`
- `capture_window(...) -> Result<()>`（Windows / WSL 側は当面 `Err("not implemented on Windows yet")`）

cfg / runtime selector:

- `cfg(target_os = "macos")` → `macos.rs`
- `cfg(target_os = "windows")` → `windows.rs`
- `cfg(target_os = "linux")` → ランタイムで `is_wsl()` 判定。true なら `windows.rs`、false なら `unsupported.rs`

WSL 判定: `/proc/sys/kernel/osrelease` を 1 回読み、`"microsoft"` または `"WSL"` を含むかを `OnceLock` キャッシュ。

## 5. Rhino.exe 探索

優先順:

1. 環境変数 `RHINO_CLI_RHINO_EXE` が指す絶対パス
2. `C:\Program Files\Rhino 8\System\Rhino.exe`
3. `C:\Program Files\Rhino 7\System\Rhino.exe`

`--app "Rhino 8"` の数字部分（正規表現 `Rhino\s*(\d+)`）でバージョン優先順を切り替える。指定があればそのバージョンを最優先で探す。

## 6. パス取り扱い（WSL）

WSL 上では Windows 形式パス (`C:\...`) を直接 `Command::new` に渡せない。`/mnt/c/...` 形式へ変換してから渡す。

- `windows_to_wsl_path("C:\\Program Files\\Rhino 8\\System\\Rhino.exe")` → `/mnt/c/Program Files/Rhino 8/System/Rhino.exe`
- 変換は純 Rust の文字列処理で実装（`wslpath` shell-out しない）
- `RHINO_CLI_RHINO_EXE` は Windows 形式（`C:\...`）を受け付け、内部で WSL 形式に正規化する。`/mnt/c/...` 形式の入力もそのまま受ける
- 存在チェック (`path.exists()`) は変換後のパスに対して行う
- `tasklist.exe` / `taskkill.exe` は WSL の PATH に自動的に含まれる（`/mnt/c/Windows/System32`）ため、パス指定不要

## 7. デタッチ起動

Windows native 上では `std::os::windows::process::CommandExt::creation_flags(CREATE_NEW_PROCESS_GROUP = 0x00000200)` でターミナル切り離し。

WSL（Linux ターゲット）からは `creation_flags` が使えないため、`spawn()` のみで起動する。Rhino 側が GUI プロセスなので、CLI が exit してもウィンドウは残る。

## 8. 公開 API への影響

CLI フラグや handler protocol には変更なし。`launch` / `shutdown` の表面挙動は同じ。

ドキュメント追従（CLAUDE.md ルールに従い同じ作業内で更新）:

- `README.md`: 「Rhino 実機確認」節を OS 中立に
- `docs/design.md`: §1.4 Non-Goals の "Windows での開発検証" 項目、§4.2.8 / §4.2.10 / §4.2.15 の表現
- ルート `CLAUDE.md`: 「macOS 実機機能…」記述を OS 別サポート表に

## 9. 依存性追加

新規クレートは入れない。`std::process::Command` と `std::fs`、`std::sync::OnceLock` のみで完結する。

## 10. スコープ外

- `screenshot` の Windows 実装（PrintWindow / BitBlt が必要、別タスク）
- Linux native（Rhino for Linux が存在しない）
- `rhino-cli plugin set-port` の config path Windows 対応（既存実装の確認は別途）

## 11. 検証手順

Windows / WSL 側で以下を実施する（タスクリスト Phase 5 と対応）:

1. `cargo make build` が Windows native / WSL の両方で通る
2. Windows native: `plugin set-port 50061` → `launch --new-model` → `wait-ready --port 50061` → `doctor` → `shutdown` が一通り成功
3. WSL: 同シナリオが成功（Windows 側 Rhino が起動・終了する）
4. 保存待ちダイアログが出るケースで `shutdown --timeout 30` が想定どおりタイムアウトする
5. `RHINO_CLI_RHINO_EXE` 上書きで Rhino 7 / Rhino 8 を切り替えられる、存在しないパスを指せば明示エラー
