# Remote Proof-of-Stake Reconciliation Finite State Machine

That's a mouthful.

Let's break it down:

- Remote: The operations are spread across 2 chains, the host chain where the state machine logic is running and the remote chain where the messages are executed.

- Proof-Of-Stake: Delegate tokens to validators to get more tokens. To get your tokens back you need to wait a period of time for them to 'unbond'.
This is so you can still be slashed if the validator misbehaves. Slashed means some of your tokens are taken away (assume lost forever) - this happens to everyone delegated and waiting to unbond.

- Reconciliation: Ensure the tokens you want / think to be delegated are actually delegated. You can have tokens waiting to be delegated, tokens waiting to
be unbonded, reward tokens waiting to be collected (and redelegated), and unbonded tokens waiting to be collected (and later claimed).

- Finite State Machine (FSM): A series of states and legal transitions that ensure the process to reconcile proceeds in the correct order. It provides a model in which to
split the logic required to maintain correct expected balances & issue messages that manipulate the real balances into smaller units.

The FSM is hierarchical, meaning that it consists of a machine within a machine, Outer and Inner. 

## Outer FSM

The following shows the different outer states or 'phases' that the FSM can be in. A 'Checkpoint' means that when reset, the FSM goes back 
to the indicated phase once that checkpoint has been reached (i.e. phase entered).

```
       ┌───────────────────────────┐<───── Checkpoint
       │                           │                            
       │    Setup Rewards Addr.    │                            
       │                           │                            
       └─────────────┬─────────────┘                            
                     │                                          
                     │                                          
       ┌─────────────▼─────────────┐                            
       │                           │                            
       │        Setup Authz        │                            
       │                           │                            
       └─────────────┬─────────────┘                            
                     │                                          
                     │                                          
       ┌─────────────▼─────────────┐<───── Checkpoint
       │                           │                            
┌──────►      Start Reconcile      │                            
│      │                           │                            
│      └─────────────┬─────────────┘                            
│                    │                                          
│                    │                                          
│      ┌─────────────▼─────────────┐                            
│      │                           │                            
│      │         Redelegate        │                            
│      │                           │                            
│      └─────────────┬─────────────┘                            
│                    │                                          
│                    │                                          
│      ┌─────────────▼─────────────┐                            
│      │                           │                            
│      │         Undelegate        │                            
│      │                           │                            
│      └─────────────┬─────────────┘                            
│                    │                                          
│                    │                                          
│      ┌─────────────▼─────────────┐                            
│      │                           │                            
│      │   Transfer Undelegated    │                            
│      │                           │                            
│      └─────────────┬─────────────┘                            
│                    │                                          
│                    │                                          
│      ┌─────────────▼─────────────┐                            
│      │                           │                            
│      │ Transfer Pending Deposits │                            
│      │                           │                            
│      └─────────────┬─────────────┘                            
│                    │                                          
│                    │                                          
│      ┌─────────────▼─────────────┐                            
│      │                           │                            
└──────┤          Delegate         │                            
       │                           │                            
       └───────────────────────────┘                            
```

## Inner FSM

Every phase contains an inner FSM in order to deal with the different events that can occur during phase execution.

The resting states are `Idle` & `Failed`, these are states when an external actor needs to trigger execution.

The `Pending` phase is awaiting an event from the platform to indicate whether an issued transaction was successful or not.

The possible external triggers are:

- `reconcile`: This is used to advance the state machine forwards.
- `reset`: Take the state machine back to the last 'checkpoint' phase (leaving it in the `Idle` state).

`next` forwards the `reconcile` trigger to the next phase if there is nothing to do.

`tx_success` is the same as an external entity triggering `reconcile` on the next phase.

```
                               reconcile                         
                                   │                             
                                   │                             
                                   │                             
                   ┌───────────────┼───────────────┐             
                   │               │               │             
                   │               │               │             
                   │      ┌────────▼────────┐      │             
                 next     │                 │      │             
       ┌───────────┬──────┤      Idle       ◄──────┼──┐          
       │           │      │                 │      │  │          
       │           │      └────────┬────────┘      │  │          
       │           │               │issue_tx       │  │          
       │           │               │               │  │          
       │           │      ┌────────▼────────┐      │  │          
┌──────▼──────┐ tx_success│                 │      │  │          
│ Next Phase  ◄────┬──────┤     Pending     │      │  │ reconcile
└──────▲──────┘    │      │                 │      │  │          
       │           │      └────────┬────────┘      │  │          
       │           │               │notify_failed  │  │          
       │           │               │               │  │          
       │           │      ┌────────▼────────┐      │  │          
       │      force_next* │                 │      │  │          
       └───────────┬──────┤     Failed      ├──────┼──┘          
                   │      │                 │      │             
                   │      └─────────────────┘      │             
                   │                               │             
                   │          Current Phase        │             
                   │                               │             
                   └───────────────────────────────┘             

  * available in the Undelegate / Delegate phase
```

## Phases

A description of what each phase should do.

`MainICA` is the interchain account (ICA) from which delegations are made.

`RewardsICA` is the ICA that rewards should be withdrawable to.

#### __Setup Rewards Addr.__

Issue an interchain transaction (ICTX) on behalf of the `MainICA` to set it's withdrawal address to the `RewardsICA`.

#### __Setup Authz__

Issue a ICTX on behalf of the `RewardsICA` to grant the `MainICA` the ability to send assets from the `RewardsICA`.

#### __Start Reconcile__

This phase issues no transactions, it checks to see if there have been any slashings since the previous reconciliation (if any). 

It does this by checking the last `MainICA` _delegations_ interchain query (ICQ) result, 
if it was posted after the previous reconcile height and the total delegations is less than the expected delegations balance, 
the loss is accounted for.

#### __Redelegate__

If there is a pending redelegation for a validator set slot, a ICTX is issued to complete the redelegation.
The pending redelegation request is always cleared when the phase is entered.
If the redelegation fails, it continues onto the next phase automatically (unlike the other phases where `Failed` is a resting state).

#### __Undelegate__

If there is a pending unbond balance, an ICTX should be issued to undelegate the specified amount from the validator set. 

#### __Transfer Undelegated__

This phase should detect if there are any assets that have finished undelegating and transfer them back to the host chain so they can be claimed.

It does this by checking the last `MainICA` _balance_ ICQ result, 
if it was posted after the previous reconcile height and is a non-zero amount, 
then that amount is assumed to be the result of a completed undelegation and transferred.

#### __Transfer Pending Deposits__

Any pending deposits should be transferred to the `MainICA` in order to be delegated.

#### __Delegate__

In the final stage, the last `RewardsICA` _balance_ ICQ result is checked to see if it was posted after the previous reconcile height and if it is non-zero.
A non-zero, non-stale balance is determined to be accrued rewards.

Depending on whether certain conditions are met, this amount will either be added in full to the transferred deposit amount (if any) or split between
redelegation and the 'reconciler' as a fee for getting the reconciliation sequence to complete.

An interchain TX is issued to delegate the pending delegation amount and send the reconciler fee (if any).
