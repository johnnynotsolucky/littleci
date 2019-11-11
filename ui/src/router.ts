import Vue from 'vue'
import Router from 'vue-router'
import Home from './views/Home.vue'
import Repository from './views/Repository.vue'

Vue.use(Router)

export default new Router({
  mode: 'history',
  base: process.env.BASE_URL,
  routes: [
    {
      path: '/',
      name: 'home',
      component: Home,
    },
    {
      path: '/repositories/:repository',
      name: 'repository',
      component: Repository,
    },
    {
      path: '/config',
      name: 'config',
      component: () => import(/* webpackChunkName: "config" */ './views/Config.vue'),
    },
    {
      path: '/about',
      name: 'about',
      component: () => import(/* webpackChunkName: "about" */ './views/About.vue'),
    },
  ],
})
