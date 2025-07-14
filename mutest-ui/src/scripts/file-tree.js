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
    });

    [...document.getElementsByClassName('ft-mutation')].map(e => {
        e.addEventListener('click', () => {
            try {
                let el = document.getElementById(e.getAttribute('data-mutation-id'));
                if (el.classList.contains('hidden')) {
                    [...document.getElementsByClassName(el.classList[0])].map(e => e.classList.add('hidden'));
                    el.classList.remove('hidden');
                }
                [...document.getElementsByTagName('tbody')].map(e => e.classList.remove('selected'));
                el.classList.add('selected');
                el.scrollIntoView();
            } catch (ex) {
                let fname = [...e.parentElement.parentElement.getElementsByClassName('node-value-wrapper')][0]
                    .getAttribute('data-file-name');
                window.open(`${fname}&mutation_id=${e.getAttribute('data-mutation-id')}`, '_self');
            }
        });
    });
});