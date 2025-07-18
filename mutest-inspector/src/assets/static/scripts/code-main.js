import { FileTree } from "./file-tree.js";
import { MutationSwitcher } from "./mutation-switcher.js";
import { collapse_and_show } from "./collapser.js";
import { openMutation, openQueryMutation } from "./mutations.js";
import { search } from "./search.js";

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

    collapse_and_show(document.getElementById('code-table')).then(r => openQueryMutation());

    // TODO: temp
    let sfcb = [...document.getElementsByClassName('search-frame-content-blocker')][0];
    let searchEl = document.getElementById('search-input');

    sfcb.addEventListener('click', () => {
        sfcb.classList.add('hidden');
    });

    document.getElementById('search-popover').addEventListener('click', e => {
        e.stopPropagation();
    });

    document.addEventListener('keypress', (e) => {
        if (e.key === '/') {
            e.preventDefault();

            sfcb.classList.toggle('hidden');
            searchEl.focus();
        }
    });

    document.addEventListener('keyup', (e) => {
        if (e.key === 'Escape' && !sfcb.classList.contains('hidden')) {
            e.preventDefault();

            searchEl.blur();
            sfcb.classList.add('hidden');
        }
    });

    searchEl.addEventListener('keyup', (e) => {
        if (e.key === 'Escape' && !sfcb.classList.contains('hidden')) {
            e.preventDefault();

            searchEl.blur();
            sfcb.classList.add('hidden');
        }
    });

    [...document.getElementsByClassName('search-mutation')].map(e => {
        e.addEventListener('click', () => {
            openMutation(e.getAttribute('data-mutation-id'), e.getAttribute('data-file-path'));
        });
    });

    let searchInput = document.getElementById('search-input');
    let checkEl = document.getElementById('use-regex');

    searchInput.addEventListener('input', () => {
        search(searchEl.value, checkEl.checked);
    });

    searchInput.addEventListener('click', () => {
        search(searchEl.value, checkEl.checked);
    });
});
