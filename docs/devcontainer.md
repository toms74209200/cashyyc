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

### VS Code Remote と WSL の非互換

VS Code が Windows 側から WSL フォルダを開いて devcontainer を作成した場合、`devcontainer.local_folder` ラベルの値は **Windows UNC パス**になる:

```
\\wsl.localhost\Ubuntu-20.04\home\user\project
```

これは devcontainer CLI が Windows 側（`process.platform === 'win32'`）で動作するため。

一方、WSL 上で `devcontainer up` を実行した場合は Linux パスを直接フィルターとして使用し、フォールバックの `findDevContainerByNormalizedLabels` も `process.platform !== 'win32'` の場合は即 `return undefined` となる（`devcontainers/cli` `src/spec-node/utils.ts`）。

結果として以下の非互換が生じる:

| 作成元 | 検索元 | `devcontainer.local_folder` の値 | 結果 |
|---|---|---|---|
| VS Code (Windows) | devcontainer-cli (Windows) | Windows UNC パス | OK |
| devcontainer-cli (WSL) | devcontainer-cli (WSL) | Linux パス | OK |
| VS Code (Windows) | devcontainer-cli (WSL) | Windows UNC パス ≠ Linux パス | **コンテナを見つけられず新規作成** |

VS Code (Windows) が作成したコンテナが存在する状態で WSL から `devcontainer up` を実行すると、既存コンテナを発見できずに**2つ目のコンテナが新規作成**される。

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

### overrideCommand のデフォルト

定義: `devcontainers/cli` `src/spec-node/singleContainer.ts`

`docker run` 時は常に `--entrypoint /bin/sh` を指定し、以下のシェルスクリプトを起動コマンドとして渡す:

```sh
echo Container started
trap "exit 0" 15
exec "$@"
while sleep 1 & wait $!; do :; done
```

`while sleep 1 & wait $!` は `sleep infinity` と異なり、SIGTERM（シグナル15）を `trap` で受け取れる。

| `overrideCommand` | 動作 |
|---|---|
| 未指定 / `true` | `exec "$@"` は no-op → ループでコンテナを維持 |
| `false` | `docker image inspect` でイメージの Entrypoint + Cmd を取得し `"$@"` として渡す → `exec` でイメージ本来のプロセスに置き換わる |

### workspaceFolder のデフォルト

`workspaceFolder` が未指定の場合のデフォルト:

```
/workspaces/<localWorkspaceFolder のフォルダ名>
```

### デフォルトワークスペースマウント

`workspaceMount` が未指定の場合のデフォルト:

```
type=bind,source=${localWorkspaceFolder},target=/workspaces/<フォルダ名>
```

`${localWorkspaceFolder}` はホスト側の絶対パス。

定義: `devcontainers/cli` `src/spec-common/injectHeadless.ts`

### remoteUser の解決順序

コンテナ内で操作を行うユーザーは以下の優先順位で決定される:

1. `devcontainer.json` の `remoteUser`
2. コンテナの Docker ラベル `devcontainer.metadata`（JSON 配列）内の `remoteUser`
3. イメージの `USER` 命令で指定されたユーザー（`docker inspect` の `.Config.User`）
4. フォールバック: `root`

`devcontainer.metadata` ラベルは features のメタデータを含む JSON 配列:

```json
[{"id": "feature:1"}, {"remoteUser": "vscode"}, {"id": "feature:2"}]
```

### DockerCompose 固有の仕様

#### コンテナの識別

DockerCompose の場合は `devcontainer.local_folder` ラベルを使わず、Docker Compose が自動付与する以下の2ラベルで識別する:

- `com.docker.compose.project=<project_name>`
- `com.docker.compose.service=<service>`

#### プロジェクト名の導出

`--project-name` に渡すプロジェクト名は以下の規則で決定される:

| devcontainer ディレクトリ | プロジェクト名 |
|---|---|
| `<cwd>/.devcontainer/` | `<cwdのフォルダ名>_devcontainer` |
| それ以外 | devcontainer ディレクトリのフォルダ名 |

いずれも小文字化し `[^a-z0-9\-_]` を除去する。

定義: `devcontainers/cli` `src/spec-node/dockerCompose.ts` `getProjectName`

#### ワークスペースマウントは注入しない

ImageConfig / DockerfileConfig では CLI がワークスペースマウントを自動注入するが、DockerComposeConfig では `workspaceMount: undefined` が仕様であり CLI は一切注入しない。ユーザーが `docker-compose.yml` の `volumes:` に記述する必要がある。

定義: `devcontainers/cli` `src/spec-common/utils.ts` `getWorkspaceConfiguration`

#### キープアライブと containerUser はオーバーライドファイルで注入

`docker run --entrypoint` ではなく、一時的な YAML オーバーライドファイルの `entrypoint:` フィールドでキープアライブスクリプトを注入する。`containerUser` が指定されている場合は同ファイルの `user:` フィールドで注入する。

```yaml
services:
  '<service>':
    entrypoint: ["/bin/sh", "-c", "<keepalive script>", "-"]
    user: <containerUser>   # containerUser が指定された場合のみ
```

### ログインシェルの解決

`docker exec` で使用するシェルは以下の優先順位で決定される（`devcontainers/cli` `src/spec-common/injectHeadless.ts`）:

1. コンテナの環境変数 `$SHELL`
2. コンテナ内の `/etc/passwd`（`getent passwd <remoteUser>`）の第7フィールド
3. フォールバック: `/bin/sh`
