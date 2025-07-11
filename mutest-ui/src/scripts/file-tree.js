'use strict';

document.addEventListener('DOMContentLoaded', () => {
    [...document.getElementsByClassName('toggle')].map(e => {
        e.addEventListener('click', () => {
            if (e.parentElement.parentElement.classList.contains('expanded')) {
                e.parentElement.parentElement.classList.remove('expanded');
            } else {
                e.parentElement.parentElement.classList.add('expanded');
            }
        });
    });

    [...document.getElementsByClassName('file')].map(e => {
        e.addEventListener('click', () => {
            window.open(e.getAttribute('data-file-name'), '_self');
        });
    })
});