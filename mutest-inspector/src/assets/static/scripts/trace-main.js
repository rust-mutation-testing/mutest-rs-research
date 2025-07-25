import { collapse_and_show } from "./collapser.js";
import { FileTree } from "./file-tree.js";

document.addEventListener('DOMContentLoaded', async () => {
    let ft = new FileTree(
        document.getElementById('file-tree-wrapper'),
        document.getElementById('file-tree'));

    ft.showTracesTab();

    await collapse_and_show(document.getElementById('code-table'));
});
