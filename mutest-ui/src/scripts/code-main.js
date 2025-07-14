import { FileTree } from "./file-tree.js";
import { MutationSwitcher } from "./mutation-switcher.js";
import { collapse } from "./collapser.js";

document.addEventListener('DOMContentLoaded', function() {
    let ft = new FileTree(
        document.getElementById('file-tree-wrapper'),
        document.getElementById('file-tree'));
    ft.init();

    let ms = new MutationSwitcher(
        document.getElementById('changer'),
        [...document.getElementsByClassName('mutation-conflict-region')],
        ft);
    ms.init();

    collapse();
});
