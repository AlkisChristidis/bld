# BLD
A simple and blazingly fast CI/CD tool.

# Features
- [x] Running a pipeline on the executing machine or on a docker container.
- [x] Client and Server mode.
- [x] Authentication using an oauth2 service (Github, Google, Microsoft etc).

# Commands
Command | Description
------- | -----------
config  | Lists bld's configuration.
init    | Initializes the bld configuration.
inspect | Inspects the contents of a pipeline on a bld server.
hist    | Fetches execution history of pipelines on a bld server.
login   | Initiates the login process for a bld server
ls      | Lists pipelines in a bld server.
monit   | Connects to a bld server to monitor the execution of a pipeline.
push    | Pushes the content of a pipeline to a bld server.
pull    | Pulls the content of a pipeline from a bld server.
rm      | Removed a pipeline from a bld server.
run     | Execute a bld pipeline.
server  | Start bld in server mode, listening to incoming build requests.
stop    | Stops a running pipeline on a server.

# Usage
```bash
# Examples of the various commands that bld exposes. In most commands that target a server,
# if a name is not provided the first server entry in the config file is selected. Additionaly
# when a command requires a pipeline name, if not provided it will target the default pipeline.

# Command to create the .bld directory and a default pipeline.
bld init

# Command to run the default pipeline.
bld run

# Command to run a specific pipeline on the local machine.
# pipeline_name should be a yaml file in the .bld directory.
bld run -p pipeline_name

# Command to run a pipeline on local machine with variables.
bld run -p pipeline_name -v VARIABLE1=value1 VARIABLE2=value2

# Command to create the .bld directory for a bld server.
bld init -s

# Command to start bld in server mode.
bld server

# Command to push a local pipeline file to a server.
bld push -p pipeline_name -s server_name

# Command to run a pipeline on a server.
bld run -p pipeline_name -s server_name

# Command to run a pipeline on a server with variables.
bld run -p pipeline_name -s server_name -v VARIABLE1=value1 VARIABLE2=value2

# Command to list pipelines of a server
bld ls
bld ls -s server_name

# Command that prints the history of runned pipelines
bld hist
bld hist -s server_name

# Command to monitor the execution of a pipeline or see the output of older runs
bld monit
bld monit -i pipeline_id -s server_name

# Command to monit a pipeline's execution output of its last run.
bld monit -p pipeline_name -s server_name

# Command to inspect the contents of a pipeline on a server
bld inspect
bld inspect -p pipeline_name -s server_name
```

# Pipeline examples
#### Default pipeline
```yaml
name: Default Pipeline
runs-on: machine
steps:
- name: echo
  exec:
  - echo 'hello world'
```

### Pipeline with environment and bld variables
```yaml
name: example pipeline with variables
runs-on: ubuntu

environment:
- AN_ENVIRONMENT_VARIABLE: 1
- another_environment_variable: hello world

variables:
- A_BLD_VARIABLE: true
- another_bld_variable: goodbye

steps:
- name: Echo environment variables
  exec:
  - echo $AN_ENVIRONMENT_VARIABLE
  - echo $another_environment_variable
  - echo bld:env:AN_ENVIRONMENT_VARIABLE
  - echo bld:env:another_environment_variable

- name: Echo bld variables
  exec:
  - echo bld:var:A_BLD_VARIABLE
  - echo bld:var:another_bld_variable
```

#### Build a dotnet core project
```yaml
name: dotnet core project pipeline
runs-on: mcr.microsoft.com/dotnet/core/sdk:3.1

variables:
- BRANCH: master
- CONFIG: release

artifacts:
- method: push
  from: /some/path
  to: /some/path/in/the/container
- method: get
  from: /some/path/in/the/container
  to: /some/path
  after: build project

steps:
- name: fetch repository
  exec:
  - git clone -b bld:var:BRANCH https://github.com/project/project.git
- name: build project
  working-dir: project
  exec:
  - dotnet build -c bld:var:CONFIG
  - cp -r bin/release/netcoreapp3.1/linux-x64/* /output
```

#### Build a node project
```yaml
name: node project pipeline
runs-on: node:12.18.3

variables:
- BRANCH: master
- SCRIPT: build

artifacts:
- method: push
  from: /some/path
  to: /some/path/in/the/container
- method: get
  from: /some/path/in/the/container
  to: /some/path
  after: build project

steps:
- name: Fetch repository
  exec:
  - git clone -b bld:var:BRANCH https://github.com/project/project.git
- name: install dependencies
  working-dir: project
  exec:
  - npm install
- name: build project
  working-dir: project
  exec:
  - npm run bld:var:SCRIPT
```

#### Pipeline that invokes other pipelines
```yaml
name: pipeline that calls other pipelines
steps:
- name: Execute dotnet core pipeline
  call:
  - dotnet_core_pipeline
- name: Execute nodejs pipeline
  call:
  - nodejs_pipeline
```

# Authentication

Server mode does not have it's own authentication method but it uses external authentication services. In the future multiple ways of
authentication will be supported. The only current method is using an existing oauth2 service (Github, Google, Microsoft etc).
Below is an example of authentication using a Github oauth2 app.

#### Configuration of client to login using github
The below example assumes that a github oauth2 app has been setup.
```yaml
local:
  docker-url: tcp://127.0.0.1:2376
remote:
  - server: local_srv
    host: 127.0.0.1
    port: 6080
    auth:
      method: oauth2
      auth-url: https://github.com/login/oauth/authorize
      token-url: https://github.com/login/oauth/access_token
      client-id: your_oauth2_app_client_id
      client-secret: your_oauth2_app_client_secret
      scopes: ["public_repo", "user:email"]
  - server: local_srv_2
    host: 127.0.0.1
    port: 6090
    same-auth-as: local_srv
```

#### Configuration of server to validate user using github
This will send a request to the provided validation url in order to fetch the user info.
```yaml
local:
    enable-server: true
    server:
      host: 127.0.0.1
      port: 6080
    auth:
      method: oauth2
      validation-url: https://api.github.com/user
    logs: .bld/logs
    db: .bld/db
    docker-url: tcp://127.0.0.1:2376
```

#### Login process
```bash
# Use the login command to generate a url that will provide you with a code and state
# tokens used for the auth process
bld login

# Or use -s to specify the server name
bld login -s local_srv

Open the printed url in a browser in order to login with the specified oauth2 provider.

https://github.com/login/oauth/authorize?response_type=code&client_id=your_oauth2_client_id&state=some_state_token&code_challenge=some_generated_code_challenge&code_challenge_method=the_code_challenge_method&redirect_uri=http%3A%2F%2F127.0.0.1%3A6080%2FauthRedirect&scope=public_repo+user%3Aemail

After logging in input both the provided code and state here.
code:

state:

# At this point by navigating to the generated url you will be able to get the code and state. Copy it to your terminal and a new
# token will be created under .bld/oauth2 directory on a file with the target server as name.
```

# TLS

#### Server configuration
Server mode can be configured to use a certificate for https and wss connections. For most cases having the server behind a battle tested reverse proxy would be best.

To configure the certificate see the below example
```yaml
local:
    server:
        host: 127.0.0.1
        port: 6080
        tls:
            cert-chain: /path/to/server/certificate
            private-key: /path/to/server/private-key
    supervisor:
        host: 127.0.0.1
        port: 7080
        tls:
            cert-chain: /path/to/supervisor/certificate
            private-key: /path/to/supervisor/private-key

```
The certificate should be of type PEM. Setting the tls option for the supervisor means that all communications between the server and the supervisor will be done using https and wss.

#### Client configuration
Connecting to a server with enabled tls, the local configuration should have the option of tls set to true, as seen in the below example.
```yaml
local:
    docker-url: tcp://127.0.0.1:2376
remote:
    - server: local_srv
      host: 127.0.0.1
      port: 6080
      tls: true
```

# What to do next
- [ ] High availability mode.
