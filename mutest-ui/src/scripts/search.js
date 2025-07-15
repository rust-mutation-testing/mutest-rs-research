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

export { search };
