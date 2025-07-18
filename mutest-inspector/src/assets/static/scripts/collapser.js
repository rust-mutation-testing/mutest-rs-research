function getExpandButtonRow(callback) {
    let tr = document.createElement('tr');

    ['detection-status', 'numbers'].map(c => {
        let td = document.createElement('td');
        td.classList.add(c, 'new');
        tr.appendChild(td);
    });

    let td = document.createElement('td');
    td.classList.add('expand-button');
    td.innerText = 'Click to expand code';
    td.addEventListener('click', e => callback(e));
    tr.appendChild(td);

    return tr;
}

/**
 * Collapses any <tbody> elements with more than 15 rows and no classes.
 * @param {HTMLElement} el
 */
async function collapse_and_show(el) {
    let collapsable = [...el.getElementsByTagName("tbody")]
        .filter(e => e.classList.length === 0 && e.childNodes.length > 15);

    for (let section of collapsable) {
        let end = section.childNodes.length - 6;

        for (let i = 5; i <= end; i++) {
            section.childNodes[i].style.display = "none";
        }

        section.insertBefore(getExpandButtonRow((e) => {
            for (let node of section.childNodes) {
                node.style.display = "";
                e.target.remove();
            }
        }), section.childNodes[end]);
    }

    el.classList.remove('hidden');
    console.log(el);
}

export { collapse_and_show };
