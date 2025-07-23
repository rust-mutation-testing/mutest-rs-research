import { FileTree } from "./file-tree.js";
import { MutationSwitcher } from "./mutation-switcher.js";
import { collapse_and_show } from "./collapser.js";
import { openQueryMutation } from "./mutations.js";

document.addEventListener('DOMContentLoaded', function() {
    let ft = new FileTree(
        document.getElementById('file-tree-wrapper'),
        document.getElementById('file-tree'));
    ft.init();

    let ms = new MutationSwitcher(
        document.getElementById('changer'),
        [...document.getElementsByClassName('mutation-conflict-region')]);
    ms.init();

    collapse_and_show(document.getElementById('code-table')).then(r => openQueryMutation());

    [...document.getElementsByClassName('show-trace-btn')].map(e => {
        e.addEventListener('click', async function (_e) {
            _e.stopPropagation();
            ft.showLoadingTracesTab();
            console.log(`/api/traces?mutation_id=${e.closest("TBODY").id}`);
            let response = await fetch(`/api/traces?mutation_id=${e.closest("TBODY").id}`);
            let text = await response.text();
            ft.setLoadedTracesTab(text);
        });
    });
});
