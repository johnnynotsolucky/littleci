<template>
  <v-card flat style="border-radius: 0;">
    <v-toolbar
      v-if="job"
      flat
      dark
    >
      <v-toolbar-title>{{ job.id }}</v-toolbar-title>
    </v-toolbar>
    <v-container
      fluid
      class="grey lighten-4 fill-height"
    >
        <pre>
        {{ this.output }}
        </pre>
    </v-container>
  </v-card>
</template>

<script lang="ts">
import Vue from 'vue'
import { Component, Watch } from 'vue-property-decorator'
import {state, Job } from '../store/state'

@Component({
  name: 'JobOutput',
})
export default class JobOutput extends Vue {
  private state = state
  private slug!: string
  private jobId!: string
  private job: Job | null = null

  private output: string | null = null

  private interval!: any | null

  @Watch('$route', { immediate: true })
  async onUrlChanged(to: any) {
    if (this.interval) {
      clearInterval(this.interval)
      this.interval = null
    }

    this.slug = to.params.repository
    this.jobId = to.params.jobId

    if (this.slug && this.jobId) {
      const getJobDetails = async () => {
        [this.job, this.output] = await Promise.all([
          this.state.getJob(this.slug, this.jobId),
          this.state.getJobOutput(this.slug, this.jobId),
        ])
      }

      await getJobDetails()
      this.interval = setInterval(async () => {
        // Only poll if the job is still running
        if (this.job) {
          if (this.job.status !== 'completed') {
            await getJobDetails()
          } else {
            clearInterval(this.interval)
          }
        }
      }, 2000)
    }
  }

  destroyed() {
    if (this.interval) {
      clearInterval(this.interval)
      this.interval = null
    }
  }
}
</script>


