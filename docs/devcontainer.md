# devcontainer

## devcontainer.json

仕様: https://containers.dev/

## コンテナの識別

devcontainer CLI はコンテナ起動時に Docker ラベル `devcontainer.local_folder` を付与する。
値は起動時の local folder の絶対パス。

定義: `devcontainers/cli` `src/spec-node/singleContainer.ts`

```typescript
export const hostFolderLabel = 'devcontainer.local_folder'; // used to label containers created from a workspace/folder
```

WSL 環境で devcontainer CLI から起動した場合、値は Linux パス（例: `/home/user/project`）になる。

実行中のコンテナの container ID は以下で取得できる:

```sh
docker ps --filter "label=devcontainer.local_folder=$(pwd)" --format "{{.ID}}"
```

## devcontainer up の処理フロー

仕様: https://containers.dev/implementors/spec/

### 設定タイプ別のイメージ取得/ビルド

| 設定タイプ | 処理 |
|---|---|
| ImageConfig | `docker pull` |
| DockerfileConfig / DockerfileBuildConfig | `docker build` |
| DockerComposeConfig | `docker compose build` + `docker compose up` |

### 設定タイプ横断の共通処理

**CommonConfig の適用**（mounts, env vars, remoteUser, labels 等）はどの設定タイプでも同じ。`docker run` 引数への変換は設定タイプと独立して実装できる。

**ライフサイクルコマンドの実行**もどの設定タイプでも同じ手順:

1. `initializeCommand`（ホスト上）
2. `onCreateCommand`
3. `updateContentCommand`
4. `postCreateCommand`
5. `postStartCommand` / `postAttachCommand`

`waitFor` で同期ポイントを制御する（デフォルト: `updateContentCommand`）。

### features の処理

features の**依存関係解決**（`dependsOn`, `installsAfter` によるトポロジカルソート）と**インストール実行**（OCI レジストリからのダウンロード + `install.sh` 実行）は独立した処理。

ImageConfig に features がある場合は features を組み込んだ Dockerfile を生成してビルドが必要。
