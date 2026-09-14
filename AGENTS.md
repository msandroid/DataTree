## Learned User Preferences

- ユーザーとのコミュニケーションは日本語で行う
- UIにハードコードするテキストは英語にする
- GitHub へのコミット／push では著者名に GitHub ユーザー名 `msandroid` を使う（本名は使わない）

## Learned Workspace Facts

- DataTree は eDirStat (MIT) を土台にした Windows 向けディスク使用量アナライザ。製品名は DataTree、内部 crate 名は `edirstat*` のまま
- 公開リポジトリ: https://github.com/msandroid/DataTree
- 上流リモート: `https://github.com/Xangelix/edirstat.git` (`upstream`)
- Windows ビルドには nightly Rust ツールチェーンが必要（`windows_by_handle`）
- 主バイナリは `target/release/datatree.exe`（互換エイリアス `edirstat.exe`）
- NTFS の MFT 直読みには管理者権限が必要。非管理者時は並列 walk にフォールバック（ステータスバーに `MFT` / `Walk`）
- DataTree 追加機能: Allocated 列、File View、CSV エクスポート、サイズ検索演算子（`*.iso`、`<100m`、`a>=1g`）、`--bench-ui` JSON ベンチ、スナップショット v4
- AI ブリッジ: `datatree-mcp`（stdio MCP + JSON CLI）。プラグインは `plugins/datatree`。VS Code 拡張は `extensions/datatree`
- MCP ツール（ページング必須、全ツリー JSON 化はしない）: `list_volumes`, `scan`, `scan_status`, `cancel_scan`, `load_snapshot`, `save_snapshot`, `summary`, `path_info`, `children`, `search`, `top`, `extensions`, `export_csv`, `open_gui`, `plan_delete`, `confirm_delete`
- 削除は必ず `plan_delete` → 人間確認 → `confirm_delete`（確認文字列 `TRASH` / `DELETE PERMANENTLY`）。完全削除は `DATATREE_ALLOW_PERMANENT_DELETE=1` のときだけ露出
- GUI バイナリの `datatree mcp` は隣の `datatree-mcp` を起動する
- README トップバナー: `assets/img/datatree-banner.png`
- スポンサーシップバナー: `assets/img/sponsor-banner.svg` → https://github.com/sponsors/msandroid
- `.github/FUNDING.yml` の GitHub Sponsors は `msandroid`
