Feature: cyyc shell

  Scenario: Start a fresh container from an Image config
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And no container exists for this config
    When running "cyyc shell"
    Then the container is running

  Scenario: Start a fresh container from an Image config with a feature
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has feature "ghcr.io/devcontainers/features/node:1"
    And no container exists for this config
    When running "cyyc shell"
    Then the container is running
    And the command "node --version" succeeds in the resulting shell

  Scenario: Start a fresh container from a Dockerfile config
    Given a devcontainer config with Dockerfile:
      """
      FROM mcr.microsoft.com/devcontainers/base:debian
      """
    And no container exists for this config
    When running "cyyc shell"
    Then the container is running

  Scenario: Start a fresh container from a Dockerfile config with a feature
    Given a devcontainer config with Dockerfile:
      """
      FROM mcr.microsoft.com/devcontainers/base:debian
      """
    And the config has feature "ghcr.io/devcontainers/features/node:1"
    And no container exists for this config
    When running "cyyc shell"
    Then the container is running
    And the command "node --version" succeeds in the resulting shell

  Scenario: Start a fresh container from a DockerfileBuild config
    Given a devcontainer config with build using Dockerfile:
      """
      FROM mcr.microsoft.com/devcontainers/base:debian
      """
    And no container exists for this config
    When running "cyyc shell"
    Then the container is running

  Scenario: Start a fresh container from a DockerfileBuild config with a feature
    Given a devcontainer config with build using Dockerfile:
      """
      FROM mcr.microsoft.com/devcontainers/base:debian
      """
    And the config has feature "ghcr.io/devcontainers/features/node:1"
    And no container exists for this config
    When running "cyyc shell"
    Then the container is running
    And the command "node --version" succeeds in the resulting shell

  Scenario: Start a fresh container from a Compose config
    Given a devcontainer config using docker-compose service "app" with image "mcr.microsoft.com/devcontainers/base:debian"
    And no container exists for this config
    When running "cyyc shell"
    Then the container is running

  Scenario: Start a fresh container from a Compose config with a feature
    Given a devcontainer config using docker-compose service "app" with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has feature "ghcr.io/devcontainers/features/node:1"
    And no container exists for this config
    When running "cyyc shell"
    Then the container is running
    And the command "node --version" succeeds in the resulting shell

  Scenario: Restart a stopped Single-type container
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And a stopped container exists for this config
    When running "cyyc shell"
    Then the container is running
    And the existing container is reused

  Scenario: Restart a stopped Compose container
    Given a devcontainer config using docker-compose service "app" with image "mcr.microsoft.com/devcontainers/base:debian"
    And a stopped container exists for this config
    When running "cyyc shell"
    Then the container is running
    And the existing container is reused

  Scenario: Open a second session into a running Image-config container
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And a running container exists for this config
    When running "cyyc shell"
    Then a new shell session is opened in the existing container

  Scenario: Open a second session into a running Dockerfile-config container
    Given a devcontainer config with Dockerfile:
      """
      FROM mcr.microsoft.com/devcontainers/base:debian
      """
    And a running container exists for this config
    When running "cyyc shell"
    Then a new shell session is opened in the existing container

  Scenario: Open a second session into a running DockerfileBuild-config container
    Given a devcontainer config with build using Dockerfile:
      """
      FROM mcr.microsoft.com/devcontainers/base:debian
      """
    And a running container exists for this config
    When running "cyyc shell"
    Then a new shell session is opened in the existing container

  Scenario: Open a second session into a running Compose container
    Given a devcontainer config using docker-compose service "app" with image "mcr.microsoft.com/devcontainers/base:debian"
    And a running container exists for this config
    When running "cyyc shell"
    Then a new shell session is opened in the existing container

  Scenario: Select a named environment
    Given a "python" devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And no container exists for this config
    When running "cyyc shell python"
    Then the container is running

  Scenario: Multiple environments exist but no name is given
    Given a "python" devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And a "rust" devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And no container exists for this config
    When running "cyyc shell"
    Then the available environment names are printed
    And the command exits with a non-zero status

  Scenario: No devcontainer config exists
    Given no devcontainer config exists
    When running "cyyc shell"
    Then the command exits with a non-zero status

  Scenario: Execute initializeCommand as string on new container
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has initializeCommand "touch .init-ran"
    And no container exists for this config
    When running "cyyc shell"
    Then the file ".init-ran" exists in the workspace

  Scenario: Execute initializeCommand as array on new container
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has initializeCommand ["touch", ".init-array-ran"]
    And no container exists for this config
    When running "cyyc shell"
    Then the file ".init-array-ran" exists in the workspace

  Scenario: initializeCommand is not run when container already exists
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And a running container exists for this config
    And the config has initializeCommand "touch .init-reuse-ran"
    When running "cyyc shell"
    Then the file ".init-reuse-ran" does not exist in the workspace

  Scenario: Execute onCreateCommand as string on new container
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has onCreateCommand "whoami > /tmp/on-create-ran"
    And no container exists for this config
    When running "cyyc shell"
    Then the file "/tmp/on-create-ran" in the container contains "vscode"

  Scenario: onCreateCommand is not rerun on second shell attach
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has onCreateCommand "sh -c 'echo ran >> /tmp/on-create-count'"
    And a running container exists for this config
    When running "cyyc shell"
    Then the command "[ $(wc -l < /tmp/on-create-count) -eq 1 ]" succeeds in the resulting shell

  Scenario: Execute onCreateCommand as array on new container
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has onCreateCommand ["touch", "/tmp/on-create-array-ran"]
    And no container exists for this config
    When running "cyyc shell"
    Then the file "/tmp/on-create-array-ran" exists in the container

  Scenario: onCreateCommand runs as remoteUser
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has remoteUser "vscode"
    And the config has onCreateCommand "whoami > /tmp/on-create-user"
    And no container exists for this config
    When running "cyyc shell"
    Then the file "/tmp/on-create-user" in the container contains "vscode"

  Scenario: Execute updateContentCommand as string on new container
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has updateContentCommand "touch /tmp/update-content-ran"
    And no container exists for this config
    When running "cyyc shell"
    Then the file "/tmp/update-content-ran" exists in the container

  Scenario: Execute updateContentCommand as array on new container
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has updateContentCommand ["touch", "/tmp/update-content-array-ran"]
    And no container exists for this config
    When running "cyyc shell"
    Then the file "/tmp/update-content-array-ran" exists in the container

  Scenario: Execute postCreateCommand as string on new container
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has postCreateCommand "touch /tmp/post-create-ran"
    And no container exists for this config
    When running "cyyc shell"
    Then the file "/tmp/post-create-ran" exists in the container

  Scenario: Execute postCreateCommand as array on new container
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has postCreateCommand ["touch", "/tmp/post-create-array-ran"]
    And no container exists for this config
    When running "cyyc shell"
    Then the file "/tmp/post-create-array-ran" exists in the container

  Scenario: Execute postStartCommand as string on new container
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has postStartCommand "touch /tmp/post-start-ran"
    And no container exists for this config
    When running "cyyc shell"
    Then the file "/tmp/post-start-ran" exists in the container

  Scenario: Execute postStartCommand as string on container restart
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And a stopped container exists for this config
    And the config has postStartCommand "touch /tmp/post-start-restart-ran"
    When running "cyyc shell"
    Then the file "/tmp/post-start-restart-ran" exists in the container

  Scenario: Execute postAttachCommand as string on new container
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has postAttachCommand "touch /tmp/post-attach-ran"
    And no container exists for this config
    When running "cyyc shell"
    Then the file "/tmp/post-attach-ran" exists in the container

  Scenario: Execute postAttachCommand as array on new container
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has postAttachCommand ["touch", "/tmp/post-attach-array-ran"]
    And no container exists for this config
    When running "cyyc shell"
    Then the file "/tmp/post-attach-array-ran" exists in the container

  Scenario: Execute postAttachCommand on running container
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And a running container exists for this config
    And the config has postAttachCommand "touch /tmp/post-attach-running-ran"
    When running "cyyc shell"
    Then the file "/tmp/post-attach-running-ran" exists in the container

  Scenario: Execute postAttachCommand on container restart
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And a stopped container exists for this config
    And the config has postAttachCommand "touch /tmp/post-attach-restart-ran"
    When running "cyyc shell"
    Then the file "/tmp/post-attach-restart-ran" exists in the container

  Scenario: waitFor onCreateCommand blocks on onCreateCommand
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has waitFor "onCreateCommand"
    And the config has onCreateCommand "touch /tmp/wait-for-on-create-ran"
    And no container exists for this config
    When running "cyyc shell"
    Then the file "/tmp/wait-for-on-create-ran" exists in the container

  Scenario: waitFor postCreateCommand blocks on postCreateCommand
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has waitFor "postCreateCommand"
    And the config has postCreateCommand "touch /tmp/wait-for-post-create-ran"
    And no container exists for this config
    When running "cyyc shell"
    Then the file "/tmp/wait-for-post-create-ran" exists in the container

  Scenario: waitFor onCreateCommand does not block on updateContentCommand
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has waitFor "onCreateCommand"
    And the config has updateContentCommand "touch /tmp/wait-for-async-update-ran"
    And no container exists for this config
    When running "cyyc shell"
    Then the file "/tmp/wait-for-async-update-ran" eventually exists in the container

  Scenario: updateRemoteUserUid syncs host UID to container user for Single config
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has remoteUser "vscode"
    And no container exists for this config
    When running "cyyc shell"
    Then the container user "vscode" UID matches the host UID

  Scenario: updateRemoteUserUid syncs host UID to container user for Compose config
    Given a devcontainer config using docker-compose service "app" with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has remoteUser "vscode"
    And no container exists for this config
    When running "cyyc shell"
    Then the container user "vscode" UID matches the host UID

  Scenario: updateRemoteUserUid false skips UID sync for Single config
    Given a devcontainer config with Dockerfile:
      """
      FROM mcr.microsoft.com/devcontainers/base:debian
      RUN usermod -u 9999 vscode
      """
    And the config has remoteUser "vscode"
    And the config has updateRemoteUserUID false
    And no container exists for this config
    When running "cyyc shell"
    Then the container user "vscode" UID is "9999"

  Scenario: updateRemoteUserUid false skips UID sync for Compose config
    Given a devcontainer config using docker-compose service "app" with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has remoteUser "vscode"
    And the config has updateRemoteUserUID false
    And no container exists for this config
    When running "cyyc shell"
    Then the container user "vscode" UID is "1000"


  Scenario: Expose a numeric appPort on loopback
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has appPort 8080
    And no container exists for this config
    When running "cyyc shell"
    Then the container has port 8080 bound to 127.0.0.1

  Scenario: Expose a string appPort mapping as-is
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has appPort "9000:9000"
    And no container exists for this config
    When running "cyyc shell"
    Then the container has port 9000 bound

  Scenario: Respect overrideFeatureInstallOrder for feature installation order
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has local features "alpha" and "beta" that log their id on install
    And the config overrides feature install order with "beta" first
    And no container exists for this config
    When running "cyyc shell"
    Then the container is running
    And the install log shows "beta" installed before "alpha"

  Scenario: when containerEnv value contains ${localWorkspaceFolder} then it is expanded
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has containerEnv "RESULT" set to "${localWorkspaceFolder}"
    And no container exists for this config
    When running "cyyc shell"
    Then the container env "RESULT" is the expansion of "${localWorkspaceFolder}"

  Scenario: when runArgs contains env ${localWorkspaceFolderBasename} then it is expanded
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has runArgs with env "RESULT" set to "${localWorkspaceFolderBasename}"
    And no container exists for this config
    When running "cyyc shell"
    Then the container env "RESULT" is the expansion of "${localWorkspaceFolderBasename}"

  Scenario: when initializeCommand contains ${containerWorkspaceFolder} then it is expanded on the host
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has initializeCommand "sh -c 'printf %s ${containerWorkspaceFolder} > .init-var-result'"
    And no container exists for this config
    When running "cyyc shell"
    Then the file ".init-var-result" in the workspace contains the expansion of "${containerWorkspaceFolder}"

  Scenario: when onCreateCommand contains ${containerWorkspaceFolderBasename} then it is expanded
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has onCreateCommand "sh -c 'printf %s ${containerWorkspaceFolderBasename} > /tmp/var-result'"
    And no container exists for this config
    When running "cyyc shell"
    Then the file "/tmp/var-result" in the container contains the expansion of "${containerWorkspaceFolderBasename}"

  Scenario: when updateContentCommand contains ${localEnv:HOME} then it is expanded
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has updateContentCommand "sh -c 'printf %s ${localEnv:HOME} > /tmp/var-result'"
    And no container exists for this config
    When running "cyyc shell"
    Then the file "/tmp/var-result" in the container contains the expansion of "${localEnv:HOME}"

  Scenario: when postCreateCommand contains ${devcontainerId} then it is expanded
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has postCreateCommand "sh -c 'printf %s ${devcontainerId} > /tmp/var-result'"
    And no container exists for this config
    When running "cyyc shell"
    Then the file "/tmp/var-result" in the container contains the expansion of "${devcontainerId}"

  Scenario: when postStartCommand contains ${localWorkspaceFolder} then it is expanded
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has postStartCommand "sh -c 'printf %s ${localWorkspaceFolder} > /tmp/var-result'"
    And no container exists for this config
    When running "cyyc shell"
    Then the file "/tmp/var-result" in the container contains the expansion of "${localWorkspaceFolder}"

  Scenario: when postAttachCommand contains ${localWorkspaceFolderBasename} then it is expanded
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has postAttachCommand "sh -c 'printf %s ${localWorkspaceFolderBasename} > /tmp/var-result'"
    And no container exists for this config
    When running "cyyc shell"
    Then the file "/tmp/var-result" in the container contains the expansion of "${localWorkspaceFolderBasename}"

  Scenario: when workspaceFolder contains ${containerWorkspaceFolder} then the exec workdir is expanded
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has workspaceFolder "/work/${containerWorkspaceFolder}"
    And the config has postCreateCommand "pwd > /tmp/wf-result"
    And no container exists for this config
    When running "cyyc shell"
    Then the file "/tmp/wf-result" in the container contains the expansion of "/work/${containerWorkspaceFolder}"

  Scenario: when workspaceFolder is set to a non-default path then the mount target remains at /workspaces/<basename>
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has workspaceFolder "/workspaces/foo"
    And no container exists for this config
    When running "cyyc shell"
    Then the container has a mount destination matching the expansion of "${containerWorkspaceFolder}"
    And the container has a mount source matching the expansion of "${localWorkspaceFolder}"

  Scenario: when mounts source is ${localWorkspaceFolder} then it is expanded
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has mounts with "type=bind,source=${localWorkspaceFolder},target=/home/vscode/extra-bind"
    And no container exists for this config
    When running "cyyc shell"
    Then the container has a mount source matching the expansion of "${localWorkspaceFolder}"
    And the container has a mount destination matching the expansion of "/home/vscode/extra-bind"

  Scenario: when mounts volume name contains ${localWorkspaceFolderBasename} then it is expanded
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has mounts with "type=volume,source=cyyc-${localWorkspaceFolderBasename},target=/extra-vol"
    And no container exists for this config
    When running "cyyc shell"
    Then the container has a mount destination matching the expansion of "/extra-vol"

  Scenario: when mounts target contains ${containerWorkspaceFolder} then it is expanded
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has mounts with "type=bind,source=/tmp,target=${containerWorkspaceFolder}-extra"
    And no container exists for this config
    When running "cyyc shell"
    Then the container has a mount destination matching the expansion of "${containerWorkspaceFolder}-extra"

  Scenario: when mounts volume name contains ${containerWorkspaceFolderBasename} then it is expanded
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has mounts with "type=volume,source=cyyc-${containerWorkspaceFolderBasename},target=/extra-vol2"
    And no container exists for this config
    When running "cyyc shell"
    Then the container has a mount destination matching the expansion of "/extra-vol2"

  Scenario: when mounts source is ${localEnv:HOME} then it is expanded
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has mounts with "type=bind,source=${localEnv:HOME},target=/extra-home"
    And no container exists for this config
    When running "cyyc shell"
    Then the container has a mount destination matching the expansion of "/extra-home"

  Scenario: when mounts volume name contains ${devcontainerId} then it is expanded
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has mounts with "type=volume,source=cyyc-${devcontainerId},target=/extra-id"
    And no container exists for this config
    When running "cyyc shell"
    Then the container has a mount destination matching the expansion of "/extra-id"

  Scenario: when workspaceMount source is ${localWorkspaceFolder} then it is expanded
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has workspaceMount "type=bind,source=${localWorkspaceFolder},target=/home/vscode/custom-ws"
    And no container exists for this config
    When running "cyyc shell"
    Then the container has a mount source matching the expansion of "${localWorkspaceFolder}"
    And the container has a mount destination matching the expansion of "/home/vscode/custom-ws"

  Scenario: when workspaceMount target contains ${localWorkspaceFolderBasename} then it is expanded
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has workspaceMount "type=bind,source=/tmp,target=/ws-${localWorkspaceFolderBasename}"
    And no container exists for this config
    When running "cyyc shell"
    Then the container has a mount destination matching the expansion of "/ws-${localWorkspaceFolderBasename}"

  Scenario: when workspaceMount target contains ${containerWorkspaceFolder} then it is expanded
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has workspaceMount "type=bind,source=/tmp,target=${containerWorkspaceFolder}-ws"
    And no container exists for this config
    When running "cyyc shell"
    Then the container has a mount destination matching the expansion of "${containerWorkspaceFolder}-ws"

  Scenario: when workspaceMount target contains ${containerWorkspaceFolderBasename} then it is expanded
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has workspaceMount "type=bind,source=/tmp,target=/ws-${containerWorkspaceFolderBasename}"
    And no container exists for this config
    When running "cyyc shell"
    Then the container has a mount destination matching the expansion of "/ws-${containerWorkspaceFolderBasename}"

  Scenario: when workspaceMount source is ${localEnv:HOME} then it is expanded
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has workspaceMount "type=bind,source=${localEnv:HOME},target=/ws-home"
    And no container exists for this config
    When running "cyyc shell"
    Then the container has a mount destination matching the expansion of "/ws-home"

  Scenario: when workspaceMount target contains ${devcontainerId} then it is expanded
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has workspaceMount "type=bind,source=/tmp,target=/ws-${devcontainerId}"
    And no container exists for this config
    When running "cyyc shell"
    Then the container has a mount destination matching the expansion of "/ws-${devcontainerId}"

  Scenario: when containerUser is ${localEnv:USER} then it is expanded
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has a Dockerfile that adds the host user
    And the config has containerUser "${localEnv:USER}"
    And no container exists for this config
    When running "cyyc shell"
    Then the container user is the expansion of "${localEnv:USER}"

  Scenario: when remoteUser is ${localEnv:USER} then it is expanded in lifecycle commands
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has a Dockerfile that adds the host user
    And the config has remoteUser "${localEnv:USER}"
    And the config has postCreateCommand "whoami > /tmp/remote-user-result"
    And no container exists for this config
    When running "cyyc shell"
    Then the file "/tmp/remote-user-result" in the container contains the expansion of "${localEnv:USER}"
