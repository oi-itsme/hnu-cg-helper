# 凭据存储

配置文件按内容分为两类处理：

| 类型 | 内容 | 存储方式 |
|------|------|---------|
| **凭据** | 账号密码、API Key | AES-256-GCM 加密 |
| **设置** | 模型、base_url、端口等 | 明文 TOML |

## 加密方案

```
配置文件 (TOML)                        加密密钥
┌────────────────────────┐        ┌──────────────────────┐
│ [ai]                   │        │ OS 密钥环             │
│ base_url = "..."   明文│        │ Windows: Credential   │
│ model = "..."      明文│        │   Manager             │
│ api_key = "<cipher>"◀─┼─AES ──│ Linux: libsecret      │
│                        │  GCM   │ (keyring crate)       │
│ [auth]                 │        └──────────────────────┘
│ password = "<cipher>"  │
└────────────────────────┘
```

- 凭据字段使用 **AES-256-GCM** 逐字段加密，密文以 base64 写入配置文件
- 加密密钥存储在 **操作系统密钥环**，通过 Rust `keyring` crate 跨平台访问：
  - **Windows**: Credential Manager
  - **Linux**: libsecret（GNOME Keyring / KDE Wallet）
- 明文设置字段始终可读写，不依赖密钥环

## 运行时行为

```
服务启动 → 探测 OS 密钥环
              ├── 可用 → 取 key → 解密凭据字段 → 完整功能
              └── 不可用 → 凭据字段不可解密
                            ├── 明文设置：正常读写
                            └── 凭据：持久化禁用，每次手动输入
```

- 密钥环不可用时，明文设置（模型、base_url 等）仍可保存
- 前端通过 `GET /api/ai/config` 获取 `has_api_key` 判断是否需要用户输入凭据

### API Key 内存 Fallback

当 OS 密钥环不可用时，API Key 通过 `session_api_key` 字段保存在进程内存中：

```
用户通过前端提交 API Key
        │
        ▼
ConfigManager::set_ai_config()
        │
        ├── 密钥环可用 → encrypt_field() → 写入 config.toml（加密持久化）
        │
        └── 密钥环不可用 → raw.ai.api_key 置空
                          └── session_api_key = Some(key)  ← 仅存内存
                                  │
                                  ▼
                            ai_api_key() 读取优先级：
                            1. 尝试解密 raw.ai.api_key（持久化）
                            2. 回退到 session_api_key（内存）
```

- 内存中的 API Key **不会写入磁盘**，服务重启后丢失
- `save_to_disk()` 保存配置文件时，`api_key` 字段为空（密钥环不可用场景）
- 前端通过 `has_api_key` 为 `false` 感知到未配置状态，提示用户输入
- 适用于临时环境、CI、无密钥环的容器/服务器场景

## 配置文件格式

```toml
[ai]
api_key = "AES_GCM_BASE64_CIPHERTEXT"  # 加密
base_url = "https://api.deepseek.com"  # 明文
model = "deepseek-v4-flash"                # 明文

[auth]
username = ""                          # 明文（预留）
password = "AES_GCM_BASE64_CIPHERTEXT" # 加密（预留）

[server]
port = 20365                            # 明文（预留）
```

## 相关端点

| 端点 | 说明 |
|------|------|
| `POST /api/ai/config` | 保存 AI 配置，凭据字段加密后写入 |
| `GET /api/ai/config` | 查询当前配置，凭据只返回是否已配置（不返回明文） |

详见 [api.md](api.md)。
