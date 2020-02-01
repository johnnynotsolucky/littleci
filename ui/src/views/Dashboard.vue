<template>
	<v-card
		flat
		style="border-radius: 0;"
	>
		<v-toolbar
			flat
			dark
		>
			<v-toolbar-title>Jobs Overview</v-toolbar-title>
		</v-toolbar>
		<v-container
			fluid
			class="grey lighten-4 fill-height"
		>
			<v-row>
				<v-layout child-flex>
					<v-data-table
						:headers="headers"
						:items="jobs"
						:disable-pagination="true"
						:hide-default-header="true"
						:hide-default-footer="true"
						@click:row="onItemClick"
						v-if="jobs"
					>
						<template v-slot:item.id="{ item }">
							{{ item.id }}
						</template>
						<template v-slot:item.status="{ item }">
							<v-chip dark>{{ item.status }}</v-chip>
						</template>
					</v-data-table>
				</v-layout>
			</v-row>
		</v-container>
	</v-card>
</template>

<script lang="ts">
import Vue from 'vue'
import { Component, Watch } from 'vue-property-decorator'
import {state, JobSummary} from '../store/state'

@Component({
	name: 'Dashboard',
})
export default class Dashboard extends Vue {
	private state = state
	private jobs: JobSummary[] = []

	private interval!: any | null

	async mounted() {
		this.jobs = await this.state.getAllJobs()

		this.interval = setInterval(async () => {
				this.jobs = await this.state.getAllJobs()
		}, 5000)
	}

	get headers(): any[] {
		return [{
				text: 'Job',
				align: 'left',
				sortable: false,
				value: 'id',
			},
			{
				text: 'Repository',
				align: 'left',
				sortable: false,
				value: 'repository_name',
			},
			{
				text: 'Added',
				align: 'left',
				sortable: false,
				value: 'created_at',
			},
			{
				text: 'Updated',
				align: 'left',
				sortable: false,
				value: 'updated_at',
			},
			{
				text: 'Status',
				align: 'right',
				sortable: false,
				value: 'status',
			},
		]
	}

	onItemClick(item: JobSummary) {
		if (item.status !== 'queued') {
			this.$router.push(`/repositories/${item.repository_slug}/jobs/${item.id}`)
		}
	}
}
</script>


