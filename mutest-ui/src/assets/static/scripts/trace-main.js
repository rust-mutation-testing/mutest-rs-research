import { collapse_and_show } from "./collapser.js";
import { FileTree } from "./file-tree.js";
import { Query } from "./query.js"

document.addEventListener('DOMContentLoaded', async () => {
    let ft = new FileTree(
        document.getElementById('file-tree-wrapper'),
        document.getElementById('file-tree'));

    ft.showTracesTab();

    await collapse_and_show(document.getElementById('code-table'));

    let query = new Query(Query.queryString());
    let response = await fetch(`/api/traces?mutation_id=${query.getAttribute('mutation_id')}`);
    let text = await response.text();
    ft.setLoadedTracesTab(text);
    ft.setupTogglesForTracesTab();
});
