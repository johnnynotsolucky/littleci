import Vue from 'vue'
import Router from 'vue-router'
import Home from './views/Home.vue'
import Repository from './views/Repository.vue'
import ManageRepositories from './views/ManageRepositories.vue'
import NewRepository from './views/NewRepository.vue'
import UpdateRepository from './views/UpdateRepository.vue'
import JobOutput from './views/JobOutput.vue'

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
      path: '/repositories/:repository/jobs/:jobId',
      name: 'job_output',
      component: JobOutput,
    },
    {
      path: '/manage/repositories',
      name: 'manage_repositories',
      component: ManageRepositories,
    },
    {
      path: '/manage/repositories/new',
      name: 'new_repository',
      component: NewRepository,
    },
    {
      path: '/manage/repositories/:repository',
      name: 'new_repository',
      component: UpdateRepository,
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
