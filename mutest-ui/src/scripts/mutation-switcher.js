'use strict';

document.addEventListener('DOMContentLoaded', () => {
    let mcrs = [...document.getElementsByClassName('mutation-conflict-region')];

    mcrs.map(e => {
        e.addEventListener('click', () => {
            mcrs.map(_e => _e.classList.remove('selected'));
            e.classList.add('selected');
            let changer = document.getElementById('changer');
            changer.classList.remove('hidden');
            let regions = document.getElementById('changer-regions');
            [...regions.children].map(e => e.classList.add('hidden'));
            console.log(e.classList[0]);
            document.getElementById(e.classList[0]).classList.remove('hidden');
        });
    });

    [...document.getElementsByClassName('mutation-wrapper')].map(e => {
        e.addEventListener('click', () => {
            let targetClass = e.getAttribute('data-target-class');
            let els = [...document.getElementsByClassName(targetClass)];
            console.log(targetClass);
            let index = [...document.getElementById(targetClass).children].indexOf(e.parentNode);
            console.log(index);
            els.map(e => e.classList.add('hidden'));
            els[index].classList.remove('hidden');
            els[index].classList.add('selected');
        });
    });

    document.getElementById('mutation-changer-close-btn').addEventListener('click', () => {
        document.getElementById('changer').classList.add('hidden');
    })
});