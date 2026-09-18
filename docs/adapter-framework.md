# 站点适配框架

本项目是"爬虫框架"的技术试行：把爬虫中**易变**的部分（页面解析规则）与**稳定**的部分（抓取协议、会话、GUI）分离，易变层用可被 AI 维护、可热重载的脚本实现。

## 架构

```
固定输入(原始HTML)                                  固定输出
      │                                               ▲
      ▼                                               │
┌─────────────┐   ┌──────────────────┐   ┌──────────────┐
│ Rust 固定层  │ → │  QuickJS 脚本层   │ → │  schema 校验  │
│ 题型嗅探     │   │  (AI 可维护)      │   │  ProblemPage- │
│ 区域提取     │   │  parse()          │   │  Output      │
│ 模式擦除兜底 │   │  buildPlan()      │   │  Submission- │
└─────────────┘   └──────────────────┘   │  Plan        │
      │            ▲ AI 修复循环           └──────────────┘
      ▼            │ (改脚本→验证→热重载)
  隐私红线：脚本层永远看不到未脱敏的页面
```

### 隐私模型

- **区域提取**（固定层，`adapter.toml` 配置）：只有命中的子树进入脚本层，外层模板 chrome（导航栏、用户名、CG AI 助手 token）整页丢弃
- **模式擦除兜底**（固定层代码）：已知隐私值（登录学号）+ educg 外链 URL + 长 token 参数正则擦除
- **脚本无网络**：提交由脚本产出"提交计划"（URL + 表单描述），宿主带会话执行 HTTP；宿主执行前强制校验计划 URL 必须落在站点源站（`CG_BASE_URL`）内，跨源一律拦截——否则外域 URL 会带走 CG 会话凭证
- **cleanHtml 基址重写**：`EngineConfig.base_url` 非空时，cleanHtml 把相对 img src / a href 改写为站点绝对地址（题面在 helper 前端渲染，相对地址不指向自身服务）
- 送给外部 LLM 修复的只有：脚本源码 + 脱敏后输入 + 错误信息。固定层失败（嗅探/区域提取）没有脱敏版本可送，直接判定为开发者边界

### 脚本沙箱（crates/adapter）

- QuickJS（rquickjs），裸引擎无网络/文件系统/定时器，模块系统不启用
- 资源限额：内存 32MB / 栈 1MB / 单次执行 5s 中断
- 宿主 API 白名单：

| 函数 | 说明 |
|------|------|
| `select(css)` | 元素句柄（非负整数），未命中 -1，非法选择器抛异常 |
| `selectAll(css)` | 元素句柄数组 |
| `text(el)` / `html(el)` | 元素文本 / innerHTML，句柄非法返回 null |
| `attr(el, name)` | 属性值，不存在返回 null |
| `cleanHtml(html)` | 白名单清洗（ammonia），剥除 style/class 等表现层 |
| `log(msg)` | 进 tracing，供修复时查看 |

### 输出契约

- `problem-*.js` 导出 `parse()` → `ProblemPageOutput { statement_html, statement_text, submission }`
  - `statement_html` 是**语义化 HTML**（只含结构标签，视觉风格由前端 `.cg-prose` 全权决定）
  - `submission` 按 `kind` 分 `file_upload` / `fill_gap`，含语言列表、填空骨架等 GUI 渲染所需信息
- `submit-*.js` 导出 `buildPlan(input)` → `SubmissionPlan { method, url, body }`
  - 输入含 parse 产出的 descriptor、用户答案/代码、wtime 耗时

### 验证与热重载

- 每个适配包自带 fixtures：`fixtures/<页面类型>/NN.html`（脱敏后的脚本输入）+ `.expected.json`（期望输出）
- 比对：字段级精确匹配，`statement_*` 空白归一化后比较
- **fixtures 是硬门槛**：某页面类型的 fixture 目录缺失或为空即验证失败（空报告不算全绿），保证"验证通过"永远意味着"真的验证过"
- `cargo test` 自动遍历 `adapters/` 全部适配包跑验证（CI 零基建）
- 热重载：文件监听 → 候选脚本跑全部 fixture → **全绿才换入注册表**，红则保留旧版（validate-then-swap）
- 失败现场留存：线上解析失败时把脱敏后的脚本输入存到用户数据目录 `failure-scenes/`

### AI 修复闭环

```
解析失败 → 失败提示页 → [让 AI 修复]
  → 后端 agent 循环（最多 3 轮预算）:
      LLM 全量重写脚本 → 沙箱验证(fixtures + 失败现场复现)
      → 绿: 落盘 + 热重载, 用户刷新即用
      → 红: 验证差异回喂, 继续
  → 预算耗尽 → 引导反馈开发者
```

修复 API：`POST /api/adapter/repair`（SSE 推送进度）。

## 适配包布局

```
adapters/hnu-cg/
├── adapter.toml      # 固定层配置：题型嗅探规则、区域选择器（仅开发者维护）
├── scripts/          # 脚本层（AI 可修复范围）
│   ├── problem-program.js / submit-program.js    # 普通编程题
│   └── problem-fillgap.js / submit-fillgap.js    # 程序填空题
└── fixtures/         # 黄金样本（脱敏，可入库）
    ├── problem-program/
    └── problem-fillgap/
```

原始抓取（含隐私）放 `fixtures/raw/`（已 gitignore），用开发工具生成 fixture：

```bash
cargo run -p hnu-cg-helper-adapter --example gen_fixtures
```

## 服务端集成

- `GET /api/.../problems/:n` 返回结构化题目视图（`page_type` + 三个输出字段）
- `POST /api/.../problems/:n/submit` 执行提交计划，返回结果页原始 HTML（前端沙箱 iframe 展示）
  - 提交描述符在服务端类型化反序列化（problem_id/assign_id 为 u64），并与 URL 路径交叉校验（assign_id 直接比对，problem_id 经题目列表「序号 → problemID」映射比对），不一致返回 422
- 适配包分发：适配包用 include_dir 编译进二进制；目录解析顺序为环境变量 `HNU_CG_ADAPTERS_DIR` → `./adapters/hnu-cg`（存在则用作开发便利）→ 用户数据目录 `adapters/hnu-cg`（缺失或内嵌版本更新时自动解包；AI 修复/热重载作用于该副本，二进制升级后重新解包同步）

## 已知边界（开发者职责，AI 修不了）

- 题型嗅探规则、区域选择器变动（上游结构性改版）
- hnu_query 输入层缺口：题目列表只认 `programList.jsp` 链接，填空题可能缺失
- 登录流程、接口协议变动

## 已知限制

- **填空题 hidden 字段值取自脱敏文档**：`descriptor.hidden_fields` 由解析脚本从脱敏后的页面收集，若某 hidden 字段的值本身含隐私串（学号、长 token），会被洗成 `[USER]`/`[CG_TOKEN]` 占位符并在提交时原样回放给 CG。当前 CG 的 hidden 字段（problemID/assignID/progLanguage 等）不含此类值，故暂未修；根治方向是提交时由固定层从原始页面重新提取真值（不过脚本层）。
