# crates.io 发布

## 快速发布

```bash
# Dry-run (验证，不实际发布)
./tools/publish.sh

# 正式发布
./tools/publish.sh --publish
```

脚本自动处理所有临时修改（空 default features、ue_plugin_embed 快照），发布完成后丢弃临时分支，主分支不受影响。

## 脚本做了什么

1. 创建临时分支 `publish/YYYYMMDD-HHMMSS`
2. 将 `uika/Cargo.toml` 和 `uika-bindings/Cargo.toml` 的 default features 改为空
3. 将 `ue_plugin/` 同步到 `uika-cli/ue_plugin_embed/`（crates.io 构建需要）
4. 按依赖顺序发布所有 crate
5. 切回主分支，删除临时分支

## 发布顺序

依赖链决定顺序（被依赖的先发）：

1. `uika-ffi`
2. `uika-macros`
3. `uika-runtime`
4. `uika-bindings`
5. `uika-ue-flags`
6. `uika-codegen`
7. `uika`
8. `uika-cli`

## 为什么需要临时修改

- **空 default features**: crates.io 用户在运行 codegen 之前没有生成的模块文件，`core`/`engine` feature 会尝试编译这些不存在的文件。
- **ue_plugin_embed**: crates.io 包不含 workspace 根目录的 `ue_plugin/`，`uika-cli` 的 `build.rs` 需要 fallback 到 crate 内的快照。

## 注意事项

- 发布前确保 `cargo login` 已认证
- 每个 crate 发布后需等约 30s 让 crates.io 索引更新（脚本自动等待）
- 版本号未变的 crate 会发布失败，需先 bump 版本号
- 临时修改**不会**留在主分支上
