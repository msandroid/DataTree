const { spawn } = require("node:child_process");
const vscode = require("vscode");

let lastSummary = null;
let statusItem;

function mcpCommand() {
  return vscode.workspace.getConfiguration("datatree").get("mcpPath") || "datatree-mcp";
}

function guiCommand() {
  return vscode.workspace.getConfiguration("datatree").get("guiPath") || "datatree";
}

function runJson(args) {
  return new Promise((resolve, reject) => {
    const child = spawn(mcpCommand(), args, { shell: process.platform === "win32" });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString();
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString();
    });
    child.on("error", (err) => reject(err));
    child.on("close", (code) => {
      if (code !== 0) {
        reject(new Error(stderr.trim() || stdout.trim() || `datatree-mcp exited ${code}`));
        return;
      }
      try {
        resolve(JSON.parse(stdout.trim()));
      } catch (err) {
        reject(new Error(`Invalid JSON from datatree-mcp: ${stdout}\n${err}`));
      }
    });
  });
}

function updateStatus(summary) {
  if (!statusItem) {
    return;
  }
  if (!summary) {
    statusItem.text = "DataTree";
    statusItem.tooltip = "DataTree disk usage";
    return;
  }
  const allocated = Number(summary.allocated || 0);
  const gib = allocated / 1024 / 1024 / 1024;
  statusItem.text = `DataTree ${summary.engine || "?"} ${gib.toFixed(1)} GiB`;
  statusItem.tooltip = `${summary.path}\nengine=${summary.engine} files=${summary.files} dirs=${summary.dirs}`;
}

async function scanFolder() {
  const picked = await vscode.window.showOpenDialog({
    canSelectFiles: false,
    canSelectFolders: true,
    canSelectMany: false,
    openLabel: "Scan with DataTree",
  });
  if (!picked || picked.length === 0) {
    return;
  }
  const folder = picked[0].fsPath;
  await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: `DataTree scanning ${folder}`,
    },
    async () => {
      const summary = await runJson(["scan", folder, "--json"]);
      lastSummary = summary;
      updateStatus(summary);
      const channel = vscode.window.createOutputChannel("DataTree");
      channel.appendLine(JSON.stringify(summary, null, 2));
      channel.show(true);
    },
  );
}

async function openGui() {
  const folder =
    (vscode.workspace.workspaceFolders && vscode.workspace.workspaceFolders[0]
      ? vscode.workspace.workspaceFolders[0].uri.fsPath
      : null) || (lastSummary && lastSummary.path);
  if (!folder) {
    vscode.window.showErrorMessage("No folder to open in DataTree.");
    return;
  }
  spawn(guiCommand(), [folder], {
    detached: true,
    stdio: "ignore",
    shell: process.platform === "win32",
  }).unref();
}

function showSummary() {
  if (!lastSummary) {
    vscode.window.showInformationMessage("No DataTree summary yet. Run DataTree: Scan Folder.");
    return;
  }
  vscode.window.showInformationMessage(
    `${lastSummary.engine}  files=${lastSummary.files}  dirs=${lastSummary.dirs}  allocated=${lastSummary.allocated}`,
  );
}

function activate(context) {
  statusItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 10);
  statusItem.command = "datatree.showSummary";
  updateStatus(null);
  statusItem.show();

  context.subscriptions.push(
    statusItem,
    vscode.commands.registerCommand("datatree.scanFolder", () =>
      scanFolder().catch((err) => vscode.window.showErrorMessage(String(err))),
    ),
    vscode.commands.registerCommand("datatree.openGui", () =>
      openGui().catch((err) => vscode.window.showErrorMessage(String(err))),
    ),
    vscode.commands.registerCommand("datatree.showSummary", showSummary),
  );

  if (vscode.lm && typeof vscode.lm.registerMcpServerDefinitionProvider === "function") {
    context.subscriptions.push(
      vscode.lm.registerMcpServerDefinitionProvider("datatree", {
        provideMcpServerDefinitions: async () => {
          const Stdio = vscode.McpStdioServerDefinition;
          if (!Stdio) {
            return [];
          }
          return [new Stdio("DataTree", mcpCommand(), ["mcp"], {})];
        },
      }),
    );
  }
}

function deactivate() {}

module.exports = { activate, deactivate };
