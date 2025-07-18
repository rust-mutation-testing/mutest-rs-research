import { openMutation } from "./mutations.js";

/**
 * Look for regex matches of the search regex.
 * @param {string} regex
 * @param {HTMLElement} el
 * @returns {boolean}
 */
function regexSearch(regex, el) {
    let re = new RegExp(regex);
    let c = [...el.childNodes];
    return re.test(el.getAttribute('data-file-path')) ||
        re.test(c[0].classList[c[0].classList.length - 1]) ||
        re.test(c[1].innerText) ||
        re.test(c[2].innerText)
}

/**
 * Look for exact matches of the search term
 * @param {string} query
 * @param {HTMLElement} el
 * @returns {boolean}
 */
function exactSearch(query, el) {
    let c = [...el.childNodes];
    return el.getAttribute('data-file-path').includes(query) ||
        c[0].classList[c[0].classList.length - 1].includes(query) ||
        c[1].innerText.includes(query) ||
        c[2].innerText.includes(query)
}

function search(query, regex = false) {
    console.log('searching');
    [...document.getElementsByClassName('search-mutation')].map(e => {
        if (exactSearch(query, e)) {
            e.classList.remove('hidden');
        } else if (regex && regexSearch(query, e)) {
            e.classList.remove('hidden');
        } else {
            e.classList.add('hidden');
        }
    });
}

document.addEventListener('DOMContentLoaded', function() {
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
