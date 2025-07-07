'use strict';

document.addEventListener('DOMContentLoaded', () => {
    [...document.getElementsByClassName('mutation-conflict-region')].map(e => {
        e.addEventListener('click', () => {
            let changer = document.getElementById('changer');
            changer.classList.remove('hidden');
            [...changer.children].map(e => e.classList.add('hidden'));
            console.log(e.classList[0]);
            document.getElementById(e.classList[0]).classList.remove('hidden');
        });
    });

    [...document.getElementsByClassName('mutation-wrapper')].map(e => {
        e.addEventListener('click', () => {
            let targetClass = e.getAttribute('data-target-class');
            let els = [...document.getElementsByClassName(targetClass)];
            let index = [...document.getElementById(targetClass).children].indexOf(e);
            els.map(e => e.classList.add('hidden'));
            els[index].classList.remove('hidden');
        });
    });
});