'use strict';

function getExpandButtonRow(callback) {
    let tr = document.createElement('tr');
    let emptyTd = document.createElement('td');
    emptyTd.classList.add('detection-status', 'new');
    tr.appendChild(emptyTd);
    emptyTd = document.createElement('td');
    emptyTd.classList.add('numbers', 'new');
    tr.appendChild(emptyTd);
    let td = document.createElement('td');
    td.classList.add('expand-button');
    td.innerText = "Click to expand code";
    td.addEventListener('click', function (e) {
        callback(e);
    })
    tr.appendChild(td);
    return tr;
}

document.addEventListener('DOMContentLoaded', () => {
    let collapsableSourceSections = [...document.getElementsByTagName("tbody")]
        .filter(e => e.classList.length === 0 && e.childNodes.length > 15);

    for (let section of collapsableSourceSections) {
        let endNode = section.childNodes.length - 6;
        for (let i = 5; i <= endNode; i++) {
            // TODO: make this actually try and find the context correctly
            section.childNodes[i].style.display = "none";
        }
        section.insertBefore(getExpandButtonRow((e) => {
            for (let node of section.childNodes) {
                node.style.display = "";
                e.target.remove();
            }
        }), section.childNodes[endNode]);
    }
});
