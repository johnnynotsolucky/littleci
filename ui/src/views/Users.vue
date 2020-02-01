<template>
	<v-card flat style="border-radius: 0;">
		<v-toolbar
			flat
			dark
		>
			<v-toolbar-title>
				Manage Users
			</v-toolbar-title>
			<v-spacer />
			<v-toolbar-items>
				<v-btn to="/manage/users/new">
					New
					<v-icon right small>fas fa-plus</v-icon>
				</v-btn>
			</v-toolbar-items>
		</v-toolbar>
		<v-container
			fluid
			class="grey lighten-4 fill-height"
		>
			<v-row>
				<v-layout child-flex>
					<v-data-table
						:headers="headers"
						:items="users"
						:items-per-page="15"
						v-if="users"
					>
						<template v-slot:item.action="{ item }">
							<v-btn class="mr-2" text icon @click="deleteUser(item.id)">
								<v-icon small>fas fa-trash</v-icon>
							</v-btn>
						</template>
					</v-data-table>
				</v-layout>
			</v-row>
		</v-container>
	</v-card>
</template>

<script lang="ts">
import Vue from 'vue'
import Component from 'vue-class-component'
import {state, User} from '../store/state'

@Component({
	name: 'ManageUsers',
})
export default class ManageUsers extends Vue {
	state = state
	users: User[] = []

	get headers(): any[] {
		return [			{
				text: 'Username',
				align: 'left',
				sortable: false,
				value: 'username',
			},
			{
				text: 'Created',
				align: 'left',
				sortable: false,
				value: 'created_at',
			},
			{
				sortable: false,
				value: 'action',
			},
		]
	}

	async mounted() {
		this.users = await this.state.getUsers()
	}

	async deleteUser(id: string) {
		await this.state.deleteUser(id)
		this.users = await this.state.getUsers()
	}
}
</script>

