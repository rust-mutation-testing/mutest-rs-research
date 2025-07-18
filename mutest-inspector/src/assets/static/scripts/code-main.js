import { FileTree } from "./file-tree.js";
import { MutationSwitcher } from "./mutation-switcher.js";
import { collapse_and_show } from "./collapser.js";
import { openQueryMutation } from "./mutations.js";

document.addEventListener('DOMContentLoaded', function() {
    let ms = new MutationSwitcher(
        document.getElementById('changer'),
        [...document.getElementsByClassName('mutation-conflict-region')]);
    ms.init();

    collapse_and_show(document.getElementById('code-table')).then(r => openQueryMutation());
});
