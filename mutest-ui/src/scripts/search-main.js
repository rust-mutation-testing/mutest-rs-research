import { openMutation } from "./mutations.js";
import { search } from "./search.js"

document.addEventListener('DOMContentLoaded', () => {
    [...document.getElementsByClassName('search-mutation')].map(e => {
        e.addEventListener('click', () => {
            openMutation(e.getAttribute('data-mutation-id'), e.getAttribute('data-file-path'));
        });
    });

    let searchEl = document.getElementById('search-input');
    let checkEl = document.getElementById('use-regex');

    searchEl.addEventListener('input', () => {
        search(searchEl.value, checkEl.checked);
    });

    checkEl.addEventListener('click', () => {
        search(searchEl.value, checkEl.checked);
    });
});