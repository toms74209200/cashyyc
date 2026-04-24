# devcontainer

## devcontainer.json

仕様: https://containers.dev/

## コンテナの識別

devcontainer CLI はコンテナ起動時に以下の Docker ラベルを付与する:

- `devcontainer.local_folder`: local folder の絶対パス
- `devcontainer.config_file`: 使用した `devcontainer.json` の絶対パス（同一プロジェクト内に複数 config がある場合の識別に使用）

`devcontainer.local_folder` の値は起動時の local folder の絶対パス。

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

### `devcontainer.config_file` ラベルのパス形式

`devcontainer.local_folder` が Windows UNC パスになる一方、`devcontainer.config_file` は **Linux/WSL パス**で設定される。VS Code (Windows) が作成したコンテナでも同様。

| ラベル | VS Code (Windows) が設定する値 |
|---|---|
| `devcontainer.local_folder` | `\\wsl.localhost\Ubuntu-20.04\home\user\project` |
| `devcontainer.config_file` | `/home/user/project/.devcontainer/server/devcontainer.json` |

この非対称性により、`--filter label=devcontainer.local_folder=<値>` AND `--filter label=devcontainer.config_file=<値>` の組み合わせは VS Code 作成コンテナの検出に失敗する。

### 既存コンテナの検出（非 Compose）

`devcontainer.config_file` の値はパス形式が統一されているため、キーのみのフィルターで候補を収集し `docker inspect` 結果に対して部分一致で判定する:

1. `docker ps --filter label=devcontainer.config_file --format "{{.ID}}"` で全候補の ID を取得
2. `docker inspect <id>...` で一括取得
3. 各コンテナについて以下の条件で一致判定:
   - `devcontainer.local_folder` のパスを `/` に正規化し、末尾のフォルダ名が `cwd` のフォルダ名と一致するか
   - `devcontainer.config_file` のパスを `/` に正規化し、`/<cwd からの相対パス>` で終わるか

## マルチ config（named config）

`.devcontainer/{name}/devcontainer.json` という形式で複数の config を持つ機能は **VS Code 独自の規約**であり、公式 devcontainer spec には定義されていない。devcontainer CLI もこの形式をサポートするが、公式仕様ではなく VS Code との互換として実装されている。

## image tag

`docker build` 時のイメージタグは以下の形式:

```
vsc-{basename(cwd)}-{fnv1a(cwd)}
```

ハッシュは `cwd` のみから生成されるため、同一プロジェクト内に複数の Dockerfile ベース config があると同じタグになる（2回目のビルドが1回目を上書きする）。これは VS Code も同じ挙動であり、spec の設計による。

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

`--project-name` に渡すプロジェクト名は **最初の compose ファイルパスを解決した後の親ディレクトリ名** から決定される。`devcontainer_dir` 自体ではない点に注意。

1. `devcontainer_dir` と最初の `dockerComposeFile` を結合し `..` を解消して絶対パスに正規化する
2. その親ディレクトリを取得する
3. 親ディレクトリが `<cwd>/.devcontainer` と一致する場合は `<cwdのフォルダ名>_devcontainer`、それ以外はそのフォルダ名を使う

いずれも小文字化し `[^a-z0-9\-_]` を除去する。

例: named config `.devcontainer/server/` + `dockerComposeFile: ["../../docker-compose.yml"]` の場合、正規化後のパスは `<cwd>/docker-compose.yml` → 親ディレクトリは `<cwd>` → プロジェクト名は `<cwdのフォルダ名>`。

定義: `devcontainers/cli` `src/spec-node/dockerCompose.ts` `getProjectName`

#### ワークスペースマウントは注入しない

ImageConfig / DockerfileConfig では CLI がワークスペースマウントを自動注入するが、DockerComposeConfig では `workspaceMount: undefined` が仕様であり CLI は一切注入しない。ユーザーが `docker-compose.yml` の `volumes:` に記述する必要がある。

定義: `devcontainers/cli` `src/spec-common/utils.ts` `getWorkspaceConfiguration`

#### 既存コンテナの再起動

既存コンテナ（停止中を含む）が見つかった場合、devcontainer-cli はビルドをスキップし `docker compose up -d --no-recreate` で起動する。

`--no-recreate` が Recreation を防ぐには Docker Compose のコンフィグハッシュが一致している必要がある。ハッシュは `com.docker.compose.project.config_files` ラベルに記録された全 compose ファイルのパスと内容から計算される。そのため devcontainer-cli は以下の手順でオーバーライドファイルを復元する:

1. 既存コンテナの `com.docker.compose.project.config_files` ラベルを読み取る
2. リスト内にオーバーライドファイル（`docker-compose.devcontainer.containerFeatures` プレフィックス）が存在し、かつディスク上に残っていれば再利用する
3. 復元できなかった場合は新規生成し、それでも `--no-recreate` を付与して起動する（Recreation が発生し得る）

定義: `devcontainers/cli` `src/spec-node/dockerCompose.ts` `startContainer`

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
