import { FileTree } from "./file-tree.js";
import { MutationSwitcher } from "./mutation-switcher.js";
import { collapse } from "./collapser.js";
import { openQueryMutation } from "./mutations.js";

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

    openQueryMutation();

    // TODO: temp
    let sfcb = [...document.getElementsByClassName('search-frame-content-blocker')][0];
    let sframe = [...document.getElementsByClassName('search-frame')][0];
    let sframeDoc = sframe.contentDocument || sframe.contentWindow.document;
    let sframeSearch = sframeDoc.getElementById('search-input');

    sfcb.addEventListener('click', () => {
        sfcb.classList.add('hidden');
    });

    document.addEventListener('keypress', (e) => {
        if (e.key === '/') {
            e.preventDefault();

            sfcb.classList.toggle('hidden');
            sframeSearch.focus();
        }
    });

    document.addEventListener('keyup', (e) => {
        if (e.key === 'Escape' && !sfcb.classList.contains('hidden')) {
            e.preventDefault();

            sframeSearch.blur();
            sfcb.classList.add('hidden');
        }
    });

    sframeSearch.addEventListener('keyup', (e) => {
        if (e.key === 'Escape' && !sfcb.classList.contains('hidden')) {
            e.preventDefault();

            sframeSearch.blur();
            sfcb.classList.add('hidden');
        }
    });
});
